use std::sync::Arc;

pub mod flash_drive_observer;
pub mod file_manager;

#[derive(Debug)]
pub enum FilesSourceType {
    FlashDrive,
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