use std::{path::PathBuf, sync::Arc};

use notify::{event::AccessKind, Watcher};

use crate::{FilesManagerSink, FilesSource, FilesSourceHandler, FilesSourceType};

#[derive(Debug, thiserror::Error)]
pub enum FileSourceFlashDriveError {
    #[error("UserMediaNotFound")]
    UserMediaNotFound,

    #[error("NotifyError")]
    NotifyError(#[from] notify::Error),

    #[error("TokioJoinError")]
    TokioJoinError(#[from] tokio::task::JoinError),
}

pub struct FileSourceFlashDrive {
    media_user_path: PathBuf,
}

pub struct FileSourceFlashDriveHandler {
    watcher_task: tokio::task::JoinHandle<()>,
    watcher: notify::INotifyWatcher,
}

impl FilesSourceHandler for FileSourceFlashDriveHandler {
    type Error = FileSourceFlashDriveError;

    async fn shutdown(self) -> Result<(), Self::Error> {
        // Drop watcher should break task loop
        drop(self.watcher); 
        self.watcher_task.await.map_err(Self::Error::from)
    }
    
    async fn await_finish(self) -> Result<(), Self::Error> {
        self.watcher_task.await.map_err(Self::Error::from)
    }
}

impl FileSourceFlashDrive {
    pub async fn new(media_user_path: PathBuf) -> Self {
        Self { media_user_path }
    }
}

impl FilesSource for FileSourceFlashDrive {
    type Handler = FileSourceFlashDriveHandler;
    type Error = FileSourceFlashDriveError;

    async fn start(self, sink: Arc<dyn FilesManagerSink>) -> Result<Self::Handler, Self::Error> {
        let (watcher_tx, watcher_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(watcher_tx)?;
        watcher.watch(&self.media_user_path, notify::RecursiveMode::Recursive)?;

        let files_manager_sink = sink.get_tx();

        let watcher_task = tokio::task::spawn_blocking(move || {
            // Dropping watcher from outside should break for loop
            for res in watcher_rx {
                match res {
                    Ok(event) => {
                        tracing::trace!("event: {:?}", event);
                        let process_event_result = match event.kind {
                            notify::EventKind::Create(_) => {
                                tracing::debug!("FLASH drive inserted.");
                                files_manager_sink.blocking_send(FilesSourceType::FlashDrive)
                            },
                            notify::EventKind::Remove(_) => {
                                tracing::debug!("FLASH drive ejected.");
                                Ok(())
                            },
                            notify::EventKind::Access(AccessKind::Open(_)) => {
                                tracing::debug!("FLASH drive access in open mode.");
                                files_manager_sink.blocking_send(FilesSourceType::FlashDrive)
                            },
                            _=> Ok(())
                        };
                        process_event_result.expect("Should send event"); // TODO replace
                    },
                    Err(e) => tracing::info!("watch error: {:?}", e),
                }
            }
        });

        Ok(Self::Handler { watcher_task, watcher })
    }
}


