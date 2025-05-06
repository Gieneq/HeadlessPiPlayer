use std::{
    fs, 
    path::{Path, PathBuf}, 
    sync::mpsc
};

use notify::{
    Event, 
    RecursiveMode, 
    Watcher
};

const TMP_PATH: &str = "/tmp";
const TMP_DIR_NAME: &str = "headlesspiplayer";

fn count_dir_items(dir_path: &PathBuf) -> usize {
    match fs::read_dir(dir_path){
        Ok(dirs) => dirs.count(),
        Err(_) => 0,
    }
}

fn recreate_dir(dir_path: &PathBuf) -> Result<(), std::io::Error> {
    if !dir_path.exists() {
        println!("Creating '{dir_path:?}' dir");
        fs::create_dir(dir_path)?;
    }

    println!("Initialy found {} items in '{:?}' dir.", count_dir_items(dir_path), dir_path);

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            println!("- Attempting to remove: '{entry_path:?},");

            if entry_path.is_dir() {
                fs::remove_dir_all(&entry_path).expect("Failed to remove subdir");
            } else {
                fs::remove_file(&entry_path).expect("Failed to remove file");
            }
        }
    }
    Ok(())
}

fn init_tmp_dir() -> PathBuf {
    let tmp_dir_path = {
        let tmp_path = Path::new(TMP_PATH);
        assert!(tmp_path.exists(), "Temporary path '{TMP_PATH}' should exist");
        tmp_path.join(TMP_DIR_NAME)
    };

    recreate_dir(&tmp_dir_path).expect("Cannot recreate tmp dir");
    tmp_dir_path
}

fn get_media_user_dir() -> PathBuf {
    let media_path = Path::new("/media");

    let mut first_entry = fs::read_dir(media_path).expect("Media path should exists");
    let entry = first_entry.next().expect("Should be at one subdir inside media");
    let entry_dir = entry.expect("Other error during accessing media subdir");
    let media_user_dir_path = entry_dir.path();
    assert!(media_user_dir_path.exists(), "User media path not exist");
    media_user_dir_path
}

fn setup_user_media_watcher(watched_path: &PathBuf) -> notify::Result<(notify::INotifyWatcher, mpsc::Receiver<notify::Result<Event>>)> {
    println!("Start watching '{watched_path:?}'...");

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    // Use recommended_watcher() to automatically select the best implementation
    // for your platform. The `EventHandler` passed to this constructor can be a
    // closure, a `std::sync::mpsc::Sender`, a `crossbeam_channel::Sender`, or
    // another type the trait is implemented for.
    let mut watcher = notify::recommended_watcher(tx)?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(watched_path, RecursiveMode::Recursive)?;
    Ok((watcher, rx))
}

fn main() {
    let _tmp_dir = init_tmp_dir();
    let user_media_dir = get_media_user_dir();
    let (_media_dir_watcher, media_dir_watcher_rx) = setup_user_media_watcher(&user_media_dir).expect("Should setup watcher");

// event: Event { kind: Create(Folder), paths: ["/media/borsuk/ENBIO"], attr:tracker: None, attr:flag: None, attr:info: None, attr:source: None }
// event: Event { kind: Remove(Folder), paths: ["/media/borsuk/ENBIO"], attr:tracker: None, attr:flag: None, attr:info: None, attr:source: None }

    for res in media_dir_watcher_rx {
        match res {
            Ok(event) => println!("event: {:?}", event),
            Err(e) => println!("watch error: {:?}", e),
        }
    }

}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_tmp_file_exists() {
        let tmp_path = Path::new(TMP_PATH);
        assert!(tmp_path.exists(), "Temporary path '{TMP_PATH}' should exist");
    }
    
    #[test]
    fn test_init_tmp_dir_should_clear_tmp_dir() {
        let tmp_dir_path = init_tmp_dir();
        assert!(tmp_dir_path.exists(), "Temporary path '{tmp_dir_path:?}' should exist");
        assert_eq!(count_dir_items(&tmp_dir_path), 0, "initialized tmp dir should be empty");

        // Create some elements
        fs::File::create(tmp_dir_path.join("file1.txt")).expect("Failed creating first file");
        fs::File::create(tmp_dir_path.join("file2.txt")).expect("Failed creating second file");
        fs::File::create(tmp_dir_path.join("file3.txt")).expect("Failed creating third file");
        assert_eq!(count_dir_items(&tmp_dir_path), 3, "initialized tmp dir should be empty");
        
        let tmp_dir_path = init_tmp_dir();
        assert!(tmp_dir_path.exists(), "Temporary path '{tmp_dir_path:?}' should exist");
        assert_eq!(count_dir_items(&tmp_dir_path), 0, "initialized tmp dir should be empty");
    }

    #[test]
    fn test_access_user_media() {
        let user_media_path = get_media_user_dir();
        println!("{user_media_path:?}");
    }
}