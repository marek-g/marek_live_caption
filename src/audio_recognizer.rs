use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, Device, Host, SampleFormat, SampleRate, Stream, StreamConfig, SupportedBufferSize,
    SupportedStreamConfigRange,
};
use futures::channel::mpsc::UnboundedReceiver;
use marek_google_speech_recognition::GoogleRecognizerFactory;
use marek_speech_recognition_api::{
    RecognitionEvent, Recognizer, RecognizerFactory, RecognizerOptions,
};
use marek_vosk_speech_recognition::{VoskModelInfo, VoskRecognizerFactory};
use std::cmp::Ordering;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Captures desktop audio and recognizes speech.
pub struct AudioRecognizer {
    audio_host: Host,
    audio_stream: Option<Stream>,

    recognizer_factory: Box<dyn RecognizerFactory>,
    recognizer: Option<Arc<Mutex<Box<dyn Recognizer + Send>>>>,
}

impl AudioRecognizer {
    pub fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let audio_host = cpal::default_host();

        // let recognizer_factory =
        //     Box::new(GoogleRecognizerFactory::new(".", "./SODALanguagePacks")?);

        let recognizer_factory = Box::new(VoskRecognizerFactory::new(vec![VoskModelInfo {
            language: "en-US".to_string(),
            folder: PathBuf::from(
                "/home/marek/Ext/Src/language/vosk_models/vosk-model-small-en-us-0.15",
                //"/home/marek/Ext/Src/language/vosk_models/vosk-model-small-pl-0.22",
            ),
        }])?);

        Ok(Self {
            audio_host,
            audio_stream: None,

            recognizer_factory,
            recognizer: None,
        })
    }

    pub async fn start(
        &mut self,
    ) -> Result<UnboundedReceiver<RecognitionEvent>, Box<dyn Error + Send + Sync>> {
        let (mut audio_device, supported_audio_configs) = self.create_audio_device()?;
        let supported_audio_config = self.get_supported_audio_config(supported_audio_configs)?;

        let (recognizer, event_receiver) = self.create_recognizer(&supported_audio_config)?;
        let recognizer = Arc::new(Mutex::new(recognizer));

        let audio_stream =
            self.create_audio_stream(&mut audio_device, supported_audio_config, &recognizer)?;

        recognizer.lock().unwrap().start().await?;
        audio_stream.play()?;

        self.recognizer = Some(recognizer);
        self.audio_stream = Some(audio_stream);

        Ok(event_receiver)
    }

    pub async fn stop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.audio_stream = None;

        if let Some(recognizer) = self.recognizer.take() {
            recognizer.lock().unwrap().stop().await?;
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn create_audio_device(
        &mut self,
    ) -> Result<(Device, Vec<SupportedStreamConfigRange>), Box<dyn Error + Send + Sync>> {
        let mut device = self
            .audio_host
            .input_devices()?
            .find(|x| x.name().map(|y| y == "pulse").unwrap_or(false));
        if device.is_none() {
            device = self.audio_host.default_input_device();
        }

        let device = device.ok_or_else(|| "Cannot find audio input device!")?;
        let supported_configs = device.supported_input_configs()?.collect::<Vec<_>>();
        Ok((device, supported_configs))
    }

    #[cfg(target_os = "windows")]
    fn create_audio_device(
        &mut self,
    ) -> Result<(Device, Vec<SupportedStreamConfigRange>), Box<dyn Error + Send + Sync>> {
        // on Windows / WASAPI we ore using output device also for recording
        let device = self
            .audio_host
            .default_output_device()
            .ok_or_else(|| "Cannot find audio input device!")?;
        let supported_configs = device.supported_output_configs()?.collect::<Vec<_>>();
        Ok((device, supported_configs))
    }

    fn get_supported_audio_config(
        &mut self,
        mut supported_audio_configs: Vec<SupportedStreamConfigRange>,
    ) -> Result<StreamConfig, Box<dyn Error + Send + Sync>> {
        let target_frequency = 48000;
        let target_format = SampleFormat::I16;

        supported_audio_configs.sort_by(|a, b| {
            // firstly, sort by sample format
            if a.sample_format() == target_format && b.sample_format() != target_format {
                return Ordering::Less;
            }

            if a.sample_format() != target_format && b.sample_format() == target_format {
                return Ordering::Greater;
            }

            // secondly, sort by number of channels
            return a.channels().cmp(&b.channels());
        });
        let best_config = supported_audio_configs
            .into_iter()
            .next()
            .ok_or_else(|| "Not found supported audio input formats!")?;

        let channels = best_config.channels() as u16;
        let frequency = target_frequency
            .max(best_config.min_sample_rate().0)
            .min(best_config.max_sample_rate().0);
        let mut buffer_size = (frequency * (channels as u32) * 2) / 25;
        if let SupportedBufferSize::Range { min, max } = best_config.buffer_size() {
            buffer_size = buffer_size.max(*min).min(*max);
        }

        Ok(StreamConfig {
            channels,
            sample_rate: SampleRate(frequency),
            buffer_size: BufferSize::Fixed(buffer_size),
        })
    }

    fn create_recognizer(
        &mut self,
        audio_config: &StreamConfig,
    ) -> Result<
        (
            Box<dyn Recognizer + Send>,
            UnboundedReceiver<RecognitionEvent>,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        let mut recognizer_options = RecognizerOptions::default();
        recognizer_options.sample_rate = audio_config.sample_rate.0 as i32;
        let (recognizer, event_receiver) = self
            .recognizer_factory
            .create_recognizer(recognizer_options)?;
        Ok((recognizer, event_receiver))
    }

    fn create_audio_stream(
        &mut self,
        device: &mut Device,
        stream_config: StreamConfig,
        recognizer: &Arc<Mutex<Box<dyn Recognizer + Send>>>,
    ) -> Result<Stream, Box<dyn Error + Send + Sync>> {
        let err_fn = move |err| {
            eprintln!("an error occurred on stream: {}", err);
        };

        let stream = device.build_input_stream(
            &stream_config,
            {
                let recognizer = recognizer.clone();
                move |data: &[i16], _: &_| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    rt.block_on(async {
                        recognizer.lock().unwrap().write(data).await.unwrap();
                    });
                }
            },
            err_fn,
            None,
        )?;
        Ok(stream)
    }
}
