use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};

use tokio::sync::Mutex;

use crate::{FilesManagerSink, FilesSourceType};

#[cfg(target_os = "linux")]
const TMP_ROOT_PATH: &str = "/tmp";

const TMP_DIR_NAME: &str = "headlesspiplayer";

#[cfg(target_os = "linux")]
const MEDIA_ROOT_PATH: &str = "/media";

#[derive(Debug, thiserror::Error)]
pub enum FilesManagerError {
    #[error("TokioIoError reason = '{0}'")]
    TokioIoError(#[from] tokio::io::Error),

    #[error("UserMediaNotFound")]
    UserMediaNotFound,
}

pub struct FilesManager {
    tmp_path: PathBuf,
    media_user_path: PathBuf,
    files_source_tx: tokio::sync::mpsc::Sender<FilesSourceType>,
    event_loop_task: tokio::task::JoinHandle<()>,
}

impl FilesManagerSink for FilesManager {
    fn get_tx(&self) -> tokio::sync::mpsc::Sender<FilesSourceType> {
        self.files_source_tx.clone()
    }
}

impl FilesManager {
    const EVENTS_CAP: usize = 32;
    pub async fn new() -> Result<Self, FilesManagerError> {
        let tmp_path = PathBuf::from(TMP_ROOT_PATH).join(TMP_DIR_NAME);

        let media_user_path = {
            let media_root = PathBuf::from(MEDIA_ROOT_PATH);
            Self::find_dir_entry_inside(&media_root, Duration::from_millis(500)).await
                .ok_or(FilesManagerError::UserMediaNotFound)?
        };

        Self::recreate_dir(&tmp_path).await?;

        let (files_source_tx, mut files_source_rx) = tokio::sync::mpsc::channel(Self::EVENTS_CAP);
        // Event loop
        let event_loop_task = tokio::spawn(async move {
            loop {
                match files_source_rx.recv().await {
                    Some(file_source_type) => {
                        tracing::info!("Event loop got: {file_source_type:?}.");
                    }
                    None => {
                        tracing::info!("Shutting down event loop");
                        break;
                    },
                }
            }
        });

        Ok(Self { tmp_path, media_user_path, files_source_tx, event_loop_task })
    }

    pub fn get_media_user_path(&self) -> PathBuf {
        self.media_user_path.clone()
    }
    
    async fn count_dir_items(dir_path: &PathBuf) -> usize {
        match tokio::fs::read_dir(dir_path).await {
            Ok(mut entries) => {
                let mut count = 0;
                while let Ok(Some(_entry)) = entries.next_entry().await {
                    count += 1;
                }
                count
            },
            Err(_) => 0,
        }
    }

    async fn recreate_dir(dir_path: &PathBuf) -> Result<(), tokio::io::Error> {
        if dir_path.exists() {
            let initial_items_count = Self::count_dir_items(dir_path).await;
            tracing::debug!("Initialy '{dir_path:?}' exists and contains {initial_items_count} items.");
            
            tokio::fs::remove_dir_all(dir_path).await?;
        } else {
            tracing::debug!("Initialy '{dir_path:?}' dir not exist.");
        }
        
        tracing::debug!("Dir '{dir_path:?}' should not exists, creating.");
        tokio::fs::create_dir(dir_path).await
    }

    async fn find_dir_entry_inside(dir_path: &PathBuf, timeout_duration: Duration) -> Option<PathBuf> {
        Self::find_entry_inside_by(
            dir_path, 
            |entry_file_type| {
                match entry_file_type {
                    Ok(file_type) => file_type.is_dir(),
                    Err(_) => false,
                }
            },
            timeout_duration
        ).await
    }

    async fn find_entry_inside_by<P>(dir_path: &PathBuf, predicate: P, timeout_duration: Duration) -> Option<PathBuf> 
    where 
        P: Fn(std::io::Result<std::fs::FileType>) -> bool
    {
        let fut = async {
            if let Ok(mut entries) = tokio::fs::read_dir(dir_path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if predicate(entry.file_type().await) {
                        tracing::debug!("Found {entry:?} in {dir_path:?}");
                        return Some(entry.path())
                    }
                }
            }
            None
        };

        match tokio::time::timeout(timeout_duration, fut).await {
            Ok(result) => result,
            Err(_) => {
                tracing::debug!("Timeout reached while searching directory.");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer() // Output for tests
            .try_init();
    }

    #[tokio::test]
    async fn test_file_manager_init() {
        init_test_tracing();

        let _file_manager = FilesManager::new().await.unwrap();
    }
}