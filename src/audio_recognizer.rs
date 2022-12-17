use std::cmp::Ordering;
use std::error::Error;
use std::sync::{Arc, Mutex};
use cpal::{BufferSize, Device, Host, SampleFormat, SampleRate, Stream, StreamConfig, SupportedBufferSize};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures::channel::mpsc::UnboundedReceiver;
use marek_google_speech_recognition::{GoogleRecognizer, GoogleRecognizerFactory};
use marek_speech_recognition_api::{RecognitionEvent, Recognizer, RecognizerFactory, RecognizerOptions};

/// Captures desktop audio and recognizes speech.
pub struct AudioRecognizer {
    audio_host: Host,
    audio_stream: Option<Stream>,

    recognizer_factory: GoogleRecognizerFactory,
    recognizer: Option<Arc<Mutex<GoogleRecognizer>>>,
}

impl AudioRecognizer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let audio_host = cpal::default_host();
        let recognizer_factory =  GoogleRecognizerFactory::new(".", "./SODALanguagePacks")?;

        Ok(Self {
            audio_host,
            audio_stream: None,

            recognizer_factory,
            recognizer: None,
        })
    }

    pub fn start(&mut self) -> Result<UnboundedReceiver<RecognitionEvent>, Box<dyn Error>> {
        let mut audio_device = self.create_audio_device()?;
        let supported_audio_config = self.get_supported_audio_config(&mut audio_device)?;

        let (recognizer, event_receiver) = self.create_recognizer(&supported_audio_config)?;
        let recognizer = Arc::new(Mutex::new(recognizer));

        let audio_stream = self.create_audio_stream(&mut audio_device, supported_audio_config, &recognizer)?;

        recognizer.lock().unwrap().start()?;
        audio_stream.play()?;

        self.recognizer = Some(recognizer);
        self.audio_stream = Some(audio_stream);

        Ok(event_receiver)
    }

    pub fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.audio_stream = None;

        if let Some(recognizer) = self.recognizer.take() {
            recognizer.lock().unwrap().stop()?;
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn create_audio_device(&mut self) -> Result<Device, Box<dyn Error>> {
        let mut device = self.audio_host.input_devices()?.find(|x| x.name().map(|y| y == "pulse").unwrap_or(false));
        if device.is_none() {
            device = self.audio_host.default_input_device();
        }

        device.ok_or_else(|| "Cannot find audio input device!".into())
    }

    #[cfg(target_os = "windows")]
    fn create_audio_device(&mut self) -> Result<Device, Box<dyn Error>> {
        // on Windows / WASAPI we ore using output device also for recording
        self.audio_host.default_output_device().ok_or_else(|| "Cannot find audio input device!".into())
    }

    fn get_supported_audio_config(&mut self, audio_device: &mut Device) -> Result<StreamConfig, Box<dyn Error>> {
        let target_frequency = 48000;
        let target_format = SampleFormat::I16;

        let mut supported_configs = audio_device.supported_input_configs()?.collect::<Vec<_>>();
        supported_configs.sort_by(|a, b| {
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
        let best_config = supported_configs.into_iter().next().ok_or_else(|| "Not found supported audio input formats!")?;

        let channels = best_config.channels() as u16;
        let frequency = target_frequency.max(best_config.min_sample_rate().0).min(best_config.max_sample_rate().0);
        let mut buffer_size = (frequency * (channels as u32) * 2) / 50;
        if let SupportedBufferSize::Range { min, max} = best_config.buffer_size() {
            buffer_size = buffer_size.max(*min).min(*max);
        }

        Ok(StreamConfig {
            channels,
            sample_rate: SampleRate(frequency),
            buffer_size: BufferSize::Fixed(buffer_size),
        })
    }

    fn create_recognizer(&mut self, audio_config: &StreamConfig) -> Result<(GoogleRecognizer, UnboundedReceiver<RecognitionEvent>), Box<dyn Error>> {
        let mut recognizer_options = RecognizerOptions::default();
        recognizer_options.channel_count = audio_config.channels as i32;
        recognizer_options.sample_rate = audio_config.sample_rate.0 as i32;
        let (recognizer, event_receiver) =
            self.recognizer_factory.create_recognizer(recognizer_options)?;
        Ok((recognizer, event_receiver))
    }

    fn create_audio_stream(&mut self, device: &mut Device, stream_config: StreamConfig, recognizer: &Arc<Mutex<GoogleRecognizer>>) -> Result<Stream, Box<dyn Error>> {
        let err_fn = move |err| {
            eprintln!("an error occurred on stream: {}", err);
        };

        let stream = device.build_input_stream(&stream_config, {
            let recognizer = recognizer.clone();
            move |data: &[i16], _: &_| {
                recognizer.lock().unwrap().write(Self::i16_to_u8_slice(data)).unwrap();
            }
        }, err_fn)?;
        Ok(stream)
    }

    fn i16_to_u8_slice(slice: &[i16]) -> &[u8] {
        let byte_len = 2*slice.len();
        unsafe {
            std::slice::from_raw_parts(
                slice.as_ptr().cast::<u8>(),
                byte_len
            )
        }
    }
}
