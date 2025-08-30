use std::{cell::RefCell, rc::Rc};

use fui_controls::*;
use fui_core::*;
use fui_macros::ui;
use futures::channel::mpsc::UnboundedReceiver;
use tokio::sync::Mutex;
use typemap::TypeMap;

use crate::audio_recognizer::{self, AudioRecognizer};
use futures_util::stream::StreamExt;
use marek_speech_recognition_api::RecognitionEvent;
use marek_translate_api::TextTranslator;
use marek_translate_locally::TranslateLocally;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

pub struct MainViewModel {
    audio_recognizer: RefCell<Option<AudioRecognizer>>,
    event_receiver: RefCell<Option<UnboundedReceiver<RecognitionEvent>>>,

    translator: RefCell<Option<Arc<Mutex<TranslateLocally>>>>,

    text: Property<String>,
}

impl MainViewModel {
    pub fn new() -> Rc<Self> {
        Rc::new(MainViewModel {
            audio_recognizer: RefCell::new(None),
            event_receiver: RefCell::new(None),
            translator: RefCell::new(None),
            text: Property::new(""),
        })
    }

    pub async fn init(self: &Rc<Self>) {
        self.text.set("Initializing...".to_string());

        let mut audio_recognizer = AudioRecognizer::new().unwrap();
        //let event_receiver = audio_recognizer.start().await.unwrap();

        let translator = Arc::new(Mutex::new(TranslateLocally::new().unwrap()));

        // tokio::spawn(event_receiver.for_each(move |ev| {
        //     let translator = translator.clone();
        //     async move {
        //         //println!("Event: {:?}", ev);

        //         if let RecognitionEvent::Recognition { text, .. } = ev {
        //             let mut translator = translator.lock().await;
        //             let text_translated = translator.translate(&text, "en", "pl").await;
        //             match text_translated {
        //                 Ok(text) => println!("{}", text),
        //                 Err(err) => println!("Err: {:?}", err),
        //             }
        //         };
        //     }
        // }));

        //sleep(Duration::from_secs(100000));

        //audio_recognizer.stop().await.unwrap();

        self.audio_recognizer
            .borrow_mut()
            .replace(AudioRecognizer::new().unwrap());
        self.translator
            .borrow_mut()
            .replace(Arc::new(Mutex::new(TranslateLocally::new().unwrap())));
    }

    pub async fn start(self: &Rc<Self>) {
        println!("--START");
        //if let (Some(audio_recognizer), Some(translator)) =
        //    (self.audio_recognizer.as_mut(), self.translator.as_mut())
        if let Some(audio_recognizer) = self.audio_recognizer.borrow_mut().as_mut() {
            let event_receiver = audio_recognizer.start().await.unwrap();
            //let translator = translator.clone();

            let text_property = self.text.clone();

            let future = event_receiver.for_each(move |ev| {
                //let translator = translator.clone();
                let mut text_property = text_property.clone();
                async move {
                    println!("Event: {:?}", ev);

                    if let RecognitionEvent::Recognition { text, .. } = ev {
                        text_property.set(text);

                        /*let mut translator = translator.lock().await;
                        let text_translated = translator.translate(&text, "en", "pl").await;
                        match text_translated {
                            Ok(text) => text_property.set(text),
                            Err(err) => println!("Err: {:?}", err),
                        }*/
                    };
                }
            });

            let handle = spawn_local_and_forget(future);

            //drop(handle);

            //tokio::time::sleep(Duration::from_secs(100000)).await;
            //sleep(Duration::from_secs(100000));

            //let ev = event_receiver;

            //self.event_receiver = Some(event_receiver);
        }
    }
}

impl ViewModel for MainViewModel {
    fn create_view(self: &Rc<Self>) -> Rc<RefCell<dyn ControlObject>> {
        ui!(
        MoveResizeArea {
            Horizontal {
                Margin: Thickness::sides(0.0f32, 5.0f32),
                Text {
                    Margin: Thickness::all(5.0f32),
                    text: &self.text,
                },
            }
        }
        )
    }
}
