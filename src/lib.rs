use std::{path::Path, sync::Arc};

pub mod flash_drive_observer;
pub mod file_manager;
pub mod video_player;
pub mod webserver;
pub mod wifi_manager;

#[derive(Debug)]
pub enum FilesSourceType {
    FlashDrive,
    UploadedVideo {
        filename: String,
        data: bytes::Bytes,
    },
}

pub trait FilesManagerSink: Send + Sync {
    fn get_tx(&self) -> tokio::sync::mpsc::Sender<FilesSourceType>;
}

pub trait FilesSource: Send + Sync {
    type Handler: FilesSourceHandler<Error = Self::Error> + Send;
    type Error: std::error::Error + Send + Sync + 'static;

    fn start(self, sink: Arc<dyn FilesManagerSink>) -> impl std::future::Future<Output = Result<Self::Handler, Self::Error>> + Send;
}

pub trait FilesSourceHandler: Send {
    type Error: std::error::Error + Send + Sync + 'static;

    fn shutdown(self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;

    fn await_finish(self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}

#[derive(Debug, thiserror::Error)]
pub enum FileSubscriberError {

}

pub trait FileSubscriber: Send + Sync {
    /// Called before a file is deleted. Subscriber must release it.
    fn on_file_about_to_be_deleted(&self) -> impl std::future::Future<Output = Result<(), FileSubscriberError>> + Send;

    /// Called when a new file is ready.
    fn on_new_file_available(&self, file_path: &Path) -> impl std::future::Future<Output = Result<(), FileSubscriberError>> + Send;
}

pub type WiFiCredentialsProcedure = fn(&[u8]) -> Result<(), Box<dyn std::error::Error>>;