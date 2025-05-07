use std::{path::PathBuf, sync::Arc, time::Duration};

use notify::{event::AccessKind, Watcher};
use tokio::sync::Mutex;

use crate::file_manager::{self, FilesManager};

#[cfg(target_os = "linux")]
const MEDIA_ROOT_PATH: &str = "/media";

#[derive(Debug, thiserror::Error)]
pub enum FlashDriveObserverError {
    #[error("UserMediaNotFound")]
    UserMediaNotFound,

    #[error("NotifyError")]
    NotifyError(#[from] notify::Error),

    #[error("TokioJoinError")]
    TokioJoinError(#[from] tokio::task::JoinError),
}

pub struct FlashDriveObserver {
    media_user_path: PathBuf,
    watcher_task: tokio::task::JoinHandle<()>,
    watcher: notify::INotifyWatcher,
}

impl FlashDriveObserver {
    pub async fn new() -> Result<FlashDriveObserver, FlashDriveObserverError> {
        let media_user_path = {
            let media_root = PathBuf::from(MEDIA_ROOT_PATH);
            file_manager::get_instance().await
                .find_dir_entry_inside(&media_root, Duration::from_millis(500)).await
                .ok_or(FlashDriveObserverError::UserMediaNotFound)?
        };

        let (watcher_tx, watcher_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(watcher_tx)?;
        watcher.watch(&media_user_path, notify::RecursiveMode::Recursive)?;

        let media_user_path_shared = media_user_path.clone();
        let watcher_task = tokio::task::spawn_blocking(move || {
            // Will break for loop if watcher_rx got dropped
            for res in watcher_rx {
                match res {
                    Ok(event) => {
                        tracing::debug!("event: {:?}", event);
                        match event.kind {
                            notify::EventKind::Create(_) => Self::on_usb_flash_disc_inserted(&media_user_path_shared),
                            notify::EventKind::Remove(_) => Self::on_usb_flash_disc_ejected(),
                            notify::EventKind::Access(AccessKind::Open(_)) => {
                                tracing::debug!("Access in open mode");
                                Self::on_usb_flash_disc_inserted(&media_user_path_shared)
                            },
                            _=> {}
                        }
                    },
                    Err(e) => tracing::info!("watch error: {:?}", e),
                }
            }
        });

        Ok(Self { media_user_path, watcher_task, watcher})
    }

    fn on_usb_flash_disc_inserted(flash_drive_root_path: &PathBuf) {
        tracing::info!("Inserted FLASH drive {flash_drive_root_path:?}");
        // if let Some(found_file_path) = get_first_flash_dive_video_path(flash_drive_root_path) {
        //     tracing::info!("Found file {found_file_path:?}");
        // }
    }

    fn on_usb_flash_disc_ejected() {
        tracing::info!("Ejected FLASH drive");
    }

    pub async fn shutdown(self) -> Result<(), FlashDriveObserverError> {
        drop(self.watcher);
        self.watcher_task.await.map_err(FlashDriveObserverError::from)
    }

    pub async fn await_finish(self) -> Result<(), FlashDriveObserverError> {
        self.watcher_task.await.map_err(FlashDriveObserverError::from)
    }
}

