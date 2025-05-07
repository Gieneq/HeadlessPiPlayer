use std::{path::{Path, PathBuf}, time::Duration};

use crate::{FilesManagerSink, FilesSourceType};

#[cfg(target_os = "linux")]
const TMP_ROOT_PATH: &str = "/tmp";

const TMP_DIR_NAME: &str = "headlesspiplayer";

#[cfg(target_os = "linux")]
const MEDIA_ROOT_PATH: &str = "/media";

const SUPPORTED_VIDEO_FILES: &[&str] = &["avi", "mp4"];

fn is_supported_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_VIDEO_FILES.iter().any(|&supported| supported.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

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
    event_loop_task: tokio::task::JoinHandle<()>, // TODO add gracefull shutdown
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
        
        let tmp_path_shared = tmp_path.clone();

        let media_user_path_shared = media_user_path.clone();

        let (files_source_tx, mut files_source_rx) = tokio::sync::mpsc::channel(Self::EVENTS_CAP);
        // Event loop
        let event_loop_task = tokio::spawn(async move {
            loop {
                match files_source_rx.recv().await {
                    Some(FilesSourceType::FlashDrive) => {
                        if let Err(e) = Self::process_files_from_flash_drive(&tmp_path_shared, &media_user_path_shared).await {
                            tracing::error!("Failed to process files from flash drive, reason = '{e}'");
                        }
                    },
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

    async fn process_files_from_flash_drive(tmp_path: &Path, media_user_path: &Path) -> Result<(), tokio::io::Error> {
        tracing::info!("Attempt to find files in FLASH drive and compy first to temporary dir.");

        // Find FLASH drive directory inside media user directory
        if let Some(flash_drive_root) = Self::find_dir_entry_inside(media_user_path, Duration::from_millis(500)).await {
            
            // Find first video file
            if let Ok(Some(video_file_path)) = Self::find_supported_video_file(&flash_drive_root).await {
                tracing::info!("Found video file in FLASH drive {video_file_path:?}.");
                
                tracing::info!("File copied. Attempt to notify subscriber: file deletion");
                // Notify subscriber about incomming file removal
                // TODO
                
                // Await subscriber is ready
                // TODO

                // Clean temporary directory
                tracing::info!("Clearing temp directory {video_file_path:?}.");
                Self::recreate_dir(tmp_path).await.inspect_err(|e| {
                    tracing::warn!("Could not recreate temp dir, reason = {e}");
                })?;

                // Copy file
                let video_file_name = video_file_path
                    .file_name()
                    .ok_or_else(|| tokio::io::Error::new(tokio::io::ErrorKind::Other, "File has no name"))?;

                // Function 'copy' requires path to file not directory
                let video_file_destination_path = tmp_path.join(video_file_name);

                tracing::info!("Attemt to copy file {video_file_path:?} to {video_file_destination_path:?}.");
                tokio::fs::copy(&video_file_path, &video_file_destination_path).await.inspect_err(|e| {
                    tracing::warn!("Could not copy file from {video_file_path:?} to {video_file_destination_path:?}, reason = {e}");
                })?;

                tracing::info!("File copied. Attempt to notify subscriber: new file available");
                debug_assert_eq!(Self::count_dir_items(tmp_path).await, 1);
                // Notify subscriber new file is ready
                // TODO
            }
        }

        Ok(())
    }

    async fn find_supported_video_file(dir: &Path) -> Result<Option<PathBuf>, tokio::io::Error> {
        let mut entries = tokio::fs::read_dir(dir).await?;
    
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
    
            if path.is_dir() {
                // Recurse into subdirectories
                if let Some(found) = Box::pin(Self::find_supported_video_file(&path)).await? {
                    return Ok(Some(found));
                }
            } else if is_supported_video_file(&path) {
                return Ok(Some(path));
            }
        }
    
        Ok(None)
    }
    
    async fn count_dir_items(dir_path: &Path) -> usize {
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

    async fn recreate_dir(dir_path: &Path) -> Result<(), tokio::io::Error> {
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

    async fn find_dir_entry_inside(dir_path: &Path, timeout_duration: Duration) -> Option<PathBuf> {
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

    async fn find_entry_inside_by<P>(dir_path: &Path, predicate: P, timeout_duration: Duration) -> Option<PathBuf> 
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