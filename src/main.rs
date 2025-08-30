mod audio_recognizer;
use fui_app::{Application, Window, WindowOptions};
use fui_core::spawn_local_and_forget;
use std::error::Error;
use tokio::task::LocalSet;

mod main_view_model;
use main_view_model::MainViewModel;

#[tokio::main()]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    LocalSet::new()
        .run_until(async {
            let app = Application::new("Marek Live Caption").await?;

            let mut window = Window::create(
                WindowOptions::new()
                    .with_stay_on_top(true)
                    .with_translucent_background(fui_system_core::TranslucentEffect::Transparent)
                    .with_frame_type(fui_system_core::WindowFrameType::Frameless)
                    .with_title("Marek Live Caption")
                    .with_size(800, 100),
            )
            .await?;

            let vm = MainViewModel::new();
            window.set_vm(vm.clone());

            spawn_local_and_forget({
                let vm = vm.clone();
                async move {
                    vm.init().await;
                    vm.start().await;
                }
            });

            app.run().await?;

            Ok(())
        })
        .await
}
