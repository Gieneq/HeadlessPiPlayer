use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};

use tokio::sync::Mutex;

#[cfg(target_os = "linux")]
const TMP_ROOT_PATH: &str = "/tmp";

const TMP_DIR_NAME: &str = "headlesspiplayer";

#[derive(Debug, thiserror::Error)]
pub enum FilesManagerError {
    #[error("TokioIoError reason = '{0}'")]
    TokioIoError(#[from] tokio::io::Error),
}


pub struct FilesManager {
    tmp_path: PathBuf
}

static FILES_MANAGER: tokio::sync::OnceCell<Mutex<FilesManager>> = tokio::sync::OnceCell::const_new();

pub async fn get_instance<'a>() -> tokio::sync::MutexGuard<'a, FilesManager> {
    let files_manager_mutex = FILES_MANAGER.get_or_init(|| async {
        let files_manager = FilesManager {
            tmp_path: PathBuf::from(TMP_ROOT_PATH).join(TMP_DIR_NAME)
        };

        files_manager.recreate_tmp_dir().await.expect("Should recreate temporary directory");
        debug_assert_eq!(files_manager.get_tmp_dir_items_count().await, 0, "Temporary dir should be cleared");

        Mutex::new(files_manager)
    }).await;

    files_manager_mutex.lock().await
}


impl FilesManager {
    pub async fn get_tmp_dir_items_count(&self) -> usize {
        Self::count_dir_items(&self.tmp_path).await
    }

    pub async fn recreate_tmp_dir(&self) -> Result<(), tokio::io::Error> {
        Self::recreate_dir(&self.tmp_path).await
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

    pub async fn find_dir_entry_inside(&self, dir_path: &PathBuf, timeout_duration: Duration) -> Option<PathBuf> {
        self.find_entry_inside_by(
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

    pub async fn find_entry_inside_by<P>(&self, dir_path: &PathBuf, predicate: P, timeout_duration: Duration) -> Option<PathBuf> 
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

        let _file_manager = get_instance().await;
    }
}