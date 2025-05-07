use std::sync::Arc;

use headless_pi_player::{file_manager::FilesManager, flash_drive_observer::FileSourceFlashDrive, FilesSource, FilesSourceHandler};

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
}

#[tokio::main]
async fn main() {
    init_tracing();

    let files_manager = FilesManager::new().await.expect("Could not create files manager");
    let media_user_path = files_manager.get_media_user_path();
    
    // files_manager is shared among Files Sources
    let files_manager = Arc::new(files_manager);

    let source_flash_drive = FileSourceFlashDrive::new(media_user_path).await
        .start(files_manager.clone()).await.expect("msg");

    source_flash_drive.await_finish().await.unwrap();
}
