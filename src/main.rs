use futures_util::stream::StreamExt;
use marek_google_speech_recognition::{GoogleRecognizer, GoogleRecognizerFactory};
use marek_speech_recognition_api::{RecognitionEvent, RecognizerOptions, Recognizer, RecognizerFactory};
use std::error::Error;
use std::time::Duration;
use std::{future};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use cpal::{BufferSize, SampleRate, Stream, StreamConfig};
use cpal::traits::{HostTrait, StreamTrait};
use cpal::traits::DeviceTrait;
use futures::channel::mpsc::UnboundedReceiver;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let frequency = 44100u32;

    let (recognizer, event_receiver) = create_recognizer(frequency)?;
    let recognizer = Arc::new(Mutex::new(recognizer));

    let audio_stream = create_audio_stream(frequency, &recognizer)?;

    tokio::spawn(event_receiver.for_each(|ev| {
        println!("Event: {:?}", ev);
        future::ready(())
    }));

    recognizer.lock().unwrap().start()?;
    audio_stream.play()?;

    sleep(Duration::from_secs(100));

    drop(audio_stream);
    recognizer.lock().unwrap().stop()?;

    Ok(())
}

fn create_recognizer(frequency: u32) -> Result<(GoogleRecognizer, UnboundedReceiver<RecognitionEvent>), Box<dyn Error>> {
    let mut recognizer_factory = GoogleRecognizerFactory::new(".", "./SODALanguagePacks")?;
    let mut recognizer_options = RecognizerOptions::default();
    recognizer_options.sample_rate = frequency as i32;
    let (recognizer, event_receiver) =
        recognizer_factory.create_recognizer(recognizer_options)?;
    Ok((recognizer, event_receiver))
}

fn create_audio_stream(frequency: u32, recognizer: &Arc<Mutex<GoogleRecognizer>>) -> Result<Stream, Box<dyn Error>> {
    let audio_host = cpal::default_host();

    let mut device = audio_host.input_devices()?.find(|x| x.name().map(|y| y == "pulse").unwrap_or(false));
    if device.is_none() {
        device = audio_host.default_input_device();
    }
    let device = device.ok_or_else(|| "Cannot find audio input device!")?;

    let stream_config = StreamConfig {
        channels: 1,
        sample_rate: SampleRate(frequency),
        buffer_size: BufferSize::Fixed(frequency / 25),
    };

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    let stream = device.build_input_stream(&stream_config, {
        let recognizer = recognizer.clone();
        move |data: &[i16], _: &_| {
            recognizer.lock().unwrap().write(to_u8_slice(data)).unwrap();
        }
    }, err_fn)?;
    Ok(stream)
}

fn to_u8_slice(slice: &[i16]) -> &[u8] {
    let byte_len = 2*slice.len();
    unsafe {
        std::slice::from_raw_parts(
            slice.as_ptr().cast::<u8>(),
            byte_len
        )
    }
}
