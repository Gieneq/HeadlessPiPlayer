use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};

use crate::{FileSubscriber, FilesManagerSink, FilesSourceType, WiFiCredentialsProcedure};

#[cfg(target_os = "linux")]
const TMP_ROOT_PATH: &str = "/tmp";

const TMP_DIR_NAME: &str = "headlesspiplayer";

#[cfg(target_os = "linux")]
const MEDIA_ROOT_PATH: &str = "/media";

const SUPPORTED_VIDEO_FILES: &[&str] = &["avi", "mp4"];

const WIFI_CFG_FILENAME: &str = "wifi_config.json";

fn is_supported_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_VIDEO_FILES.iter().any(|&supported| supported.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

fn is_supported_wifi_credentials_file(path: &Path) -> bool {
    path.ends_with(WIFI_CFG_FILENAME)
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
    pub async fn new<S: FileSubscriber + 'static>(
        subscriber: Option<Arc<S>>,
        wifi_manager_procedure: Option<WiFiCredentialsProcedure>,
    ) -> Result<Self, FilesManagerError> {
        let tmp_path = PathBuf::from(TMP_ROOT_PATH).join(TMP_DIR_NAME);

        tracing::info!("Finding media user path");
        let media_user_path = {
            let media_root = PathBuf::from(MEDIA_ROOT_PATH);
            Self::find_dir_entry_inside(&media_root, Duration::from_millis(500)).await
                .ok_or(FilesManagerError::UserMediaNotFound)?
        };

        tracing::info!("Attempt to recreate temporary directory");
        Self::recreate_dir(&tmp_path).await?;
        
        let tmp_path_shared = tmp_path.clone();

        let media_user_path_shared = media_user_path.clone();

        let (files_source_tx, mut files_source_rx) = tokio::sync::mpsc::channel(Self::EVENTS_CAP);
        // Event loop
        let event_loop_task = tokio::spawn(async move {
            tracing::info!("Starting FilesManager event loop");
            loop {
                match files_source_rx.recv().await {
                    Some(FilesSourceType::FlashDrive) => {
                        if let Err(e) = Self::process_files_from_flash_drive(
                            &subscriber,
                            wifi_manager_procedure,
                            &tmp_path_shared, 
                            &media_user_path_shared
                        ).await {
                            tracing::error!("Failed to process files from flash drive, reason = '{e}'");
                        }
                    },
                    Some(FilesSourceType::UploadedVideo { filename, data }) => {
                        if let Err(e) = Self::process_files_from_webserver(
                            &subscriber,
                            &tmp_path_shared,
                            &filename,
                            data
                        ).await {
                            tracing::error!("Failed to save uploaded video: {e}");
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
    
    async fn process_files_from_webserver<S: FileSubscriber>(
        subscriber: &Option<Arc<S>>,  
        tmp_path: &Path, 
        filename: &str,
        data: bytes::Bytes
    ) -> Result<(), tokio::io::Error> {
        tracing::info!("Attempt to save data received by webserver.");

        tracing::info!("Attempt to notify subscriber: file deletion");
        // Notify & await subscriber response about incomming file removal
        if let Some(subs) = subscriber {
            if let Err(e) = subs.on_file_about_to_be_deleted().await {
                tracing::warn!("'on_file_about_to_be_deleted' failed reason {e}");
            }
        }
        // Clean temporary directory
        tracing::info!("Clearing temp directory {tmp_path:?}.");
        Self::recreate_dir(tmp_path).await.inspect_err(|e| {
            tracing::warn!("Could not recreate temp dir, reason = {e}");
        })?;

        // Save file
        let save_path = tmp_path.join(filename);
        tokio::fs::write(&save_path, data).await
            .inspect_err(|e| {
                tracing::error!("Failed to save file from webserver, reason {e}");
            })?;

        // Notify subscriber new file is ready
        if let Some(subs) = subscriber {
            if let Err(e) = subs.on_new_file_available(&save_path).await {
                tracing::warn!("'on_new_file_available' failed reason {e}");
            }
        }

        Ok(())
    }

    async fn process_files_from_flash_drive<S: FileSubscriber>(
        subscriber: &Option<Arc<S>>,  
        wifi_manager_procedure: Option<WiFiCredentialsProcedure>,
        tmp_path: &Path, 
        media_user_path: &Path
    ) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Attempt to find files in FLASH drive");

        // Find FLASH drive directory inside media user directory
        if let Some(flash_drive_root) = Self::find_dir_entry_inside(media_user_path, Duration::from_millis(500)).await {
            tracing::debug!("Found FLASH drive root dir: {flash_drive_root:?}.");
            Self::find_any_video_file_notify_subscriber(subscriber, tmp_path, &flash_drive_root).await?;
            Self::find_wifi_credentials_file(wifi_manager_procedure, &flash_drive_root).await?;
        }

        Ok(())
    }

    async fn find_any_video_file_notify_subscriber<S: FileSubscriber>(subscriber: &Option<Arc<S>>, tmp_path: &Path, flash_drive_root: &Path) -> Result<(), std::io::Error> {
        tracing::debug!("Attempt to find video files.");

        if let Some(video_file_path) = Self::find_supported_video_file(flash_drive_root, Duration::from_millis(2500)).await {
            tracing::info!("Found video file in FLASH drive {video_file_path:?}.");
        
            tracing::info!("Attempt to notify subscriber: file deletion");
            // Notify & await subscriber response about incomming file removal
            if let Some(subs) = subscriber {
                if let Err(e) = subs.on_file_about_to_be_deleted().await {
                    tracing::warn!("'on_file_about_to_be_deleted' failed reason {e}");
                }
            }

            // Clean temporary directory
            tracing::info!("Clearing temp directory {tmp_path:?}.");
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
            if let Some(subs) = subscriber {
                if let Err(e) = subs.on_new_file_available(&video_file_destination_path).await {
                    tracing::warn!("'on_new_file_available' failed reason {e}");
                }
            }
        } else {
            tracing::info!("Not found any files :(");
        }
        Ok(())
    }

    async fn find_wifi_credentials_file(wifi_manager_procedure: Option<WiFiCredentialsProcedure>, flash_drive_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        tracing::debug!("Attempt to find wifi credentials files.");

        if let Some(wifi_credentials_file_path) = Self::find_supported_wifi_credentials_file(flash_drive_root, Duration::from_millis(2500)).await {
            tracing::info!("Found wifi credentials file in FLASH drive {wifi_credentials_file_path:?}.");
            let content = tokio::fs::read(&wifi_credentials_file_path).await?;
            if let Some(wifi_manager_procedure) = wifi_manager_procedure {
                wifi_manager_procedure(&content)?
            }
        }
            
        Ok(())
    }

    async fn find_file_named(dir: &Path, file_name: &str, timeout_duration: Duration) -> Option<PathBuf> {
        Self::find_file_by(dir, |entry_path| {
            entry_path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == file_name)
                .unwrap_or(false)
        }, timeout_duration).await
    }

    async fn find_supported_video_file(dir: &Path, timeout_duration: Duration) -> Option<PathBuf> {
        Self::find_file_by(dir, is_supported_video_file, timeout_duration).await
    }    

    async fn find_supported_wifi_credentials_file(dir: &Path, timeout_duration: Duration) -> Option<PathBuf> {
        Self::find_file_by(dir, is_supported_wifi_credentials_file, timeout_duration).await
    }    

    async fn find_file_by<P: Fn(&Path) -> bool>(dir: &Path, predicate: P, timeout_duration: Duration) -> Option<PathBuf> {
        let fut = async {
            loop {
                let mut entries = match tokio::fs::read_dir(dir).await {
                    Ok(entries) => entries,
                    Err(e) => {
                        // Directory may not exist yet (e.g. mount race), log and retry
                        tracing::debug!("Cannot read dir {:?}: {:?}", dir, e);
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                };
    
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_file() && predicate(&path) {
                        return Some(path)
                    }
                }
    
                // Nothing found yet — wait before trying again
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        };
    
        match tokio::time::timeout(timeout_duration, fut).await {
            Ok(result) => result,
            Err(_) => {
                tracing::debug!("Timeout reached while searching video file in {:?}", dir);
                None
            }
        }
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
            
            tokio::fs::remove_dir_all(dir_path).await
                .inspect_err(|e| tracing::error!("Connot remove dirs {dir_path:?} reasone {e}."))?;

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
    use crate::video_player::VideoPlayer;

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

        let _file_manager = FilesManager::new::<VideoPlayer>(None, None).await.unwrap();
    }
}