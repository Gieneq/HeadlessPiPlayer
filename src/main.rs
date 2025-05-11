use std::sync::Arc;

use headless_pi_player::{file_manager::FilesManager, flash_drive_observer::FileSourceFlashDrive, video_player::VideoPlayer, FilesSource, FilesSourceHandler};

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
}

#[tokio::main]
async fn main() {
    init_tracing();

    let video_player = VideoPlayer::run(true).await;
    let video_player = Arc::new(video_player);

    let files_manager = FilesManager::new::<VideoPlayer>(Some(video_player)).await.expect("Could not create files manager");
    let media_user_path = files_manager.get_media_user_path();

    // Spawn shutdown signal
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let notify_clone = shutdown_notify.clone();

    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        tracing::info!("Ctrl+C received, shutting down...");
        notify_clone.notify_one();
    })
    .expect("Error setting Ctrl+C handler");
    
    // files_manager is shared among Files Sources
    let files_manager = Arc::new(files_manager);

    let source_flash_drive = FileSourceFlashDrive::new(media_user_path).await
        .start(files_manager.clone()).await.expect("msg");

    // Wait for Ctrl+C
    shutdown_notify.notified().await;

    // Gracefully shut down
    source_flash_drive.shutdown().await.expect("Failed to shut down FLASH drive source");
    tracing::info!("Shutdown complete.");
}
