mod audio_recognizer;
use crate::audio_recognizer::AudioRecognizer;
use futures_util::lock::Mutex;
use futures_util::stream::StreamExt;
use marek_speech_recognition_api::RecognitionEvent;
use marek_translate_api::TextTranslator;
use marek_translate_locally::TranslateLocally;
use std::error::Error;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut audio_recognizer = AudioRecognizer::new()?;
    let event_receiver = audio_recognizer.start().await?;

    let translator = Arc::new(Mutex::new(TranslateLocally::new()?));

    tokio::spawn(event_receiver.for_each(move |ev| {
        let translator = translator.clone();
        async move {
            //println!("Event: {:?}", ev);

            if let RecognitionEvent::Recognition { text, .. } = ev {
                let mut translator = translator.lock().await;
                let text_translated = translator.translate(&text, "en", "pl").await;
                match text_translated {
                    Ok(text) => println!("{}", text),
                    Err(err) => println!("Err: {:?}", err),
                }
            };
        }
    }));

    sleep(Duration::from_secs(100000));

    audio_recognizer.stop().await?;

    Ok(())
}
