mod audio_recognizer;

use futures_util::stream::StreamExt;
use std::error::Error;
use std::time::Duration;
use std::{future};
use std::thread::sleep;
use crate::audio_recognizer::AudioRecognizer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut audio_recognizer = AudioRecognizer::new()?;
    let event_receiver = audio_recognizer.start()?;

    tokio::spawn(event_receiver.for_each(|ev| {
        println!("Event: {:?}", ev);
        future::ready(())
    }));

    sleep(Duration::from_secs(100));

    audio_recognizer.stop()?;

    Ok(())
}
