use headless_pi_player::flash_drive_observer::FlashDriveObserver;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
}

#[tokio::main]
async fn main() {
    init_tracing();

    let flash_drive_observer = FlashDriveObserver::new().await.expect("Flash Drive Observer should work");

    flash_drive_observer.await_finish().await.unwrap();
}
