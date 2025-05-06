#[derive(Debug, thiserror::Error)]
pub enum FlashDriveObserverError {

}

pub struct FlashDriveObserver {

}

impl FlashDriveObserver {
    pub async fn new() -> Result<FlashDriveObserver, FlashDriveObserverError> {
        todo!()
    }
}


// fn setup_user_media_watcher(watched_path: &PathBuf) -> notify::Result<(notify::INotifyWatcher, mpsc::Receiver<notify::Result<Event>>)> {
//     println!("Start watching '{watched_path:?}'...");

//     let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

//     // Use recommended_watcher() to automatically select the best implementation
//     // for your platform. The `EventHandler` passed to this constructor can be a
//     // closure, a `std::sync::mpsc::Sender`, a `crossbeam_channel::Sender`, or
//     // another type the trait is implemented for.
//     let mut watcher = notify::recommended_watcher(tx)?;

//     // Add a path to be watched. All files and directories at that path and
//     // below will be monitored for changes.
//     watcher.watch(watched_path, RecursiveMode::Recursive)?;
//     Ok((watcher, rx))
// }

// fn on_usb_flash_disc_inserted(flash_drive_root_path: &PathBuf) {
//     println!("Inserted FLASH drive {flash_drive_root_path:?}");
//     if let Some(found_file_path) = get_first_flash_dive_video_path(flash_drive_root_path) {
//         println!("Found file {found_file_path:?}");
//     }
// }

// fn on_usb_flash_disc_ejected(flash_drive_root_path: &PathBuf) {
//     println!("Ejected FLASH drive {flash_drive_root_path:?}");
// }


//     let (_media_dir_watcher, media_dir_watcher_rx) = setup_user_media_watcher(&user_media_dir).expect("Should setup watcher");

//     for res in media_dir_watcher_rx {
//         match res {
//             Ok(event) => {
//                 println!("event: {:?}", event);
//                 match event.kind {
//                     notify::EventKind::Create(_) => on_usb_flash_disc_inserted(event.paths.first().expect("Should be at least one path")),
//                     notify::EventKind::Remove(_) => on_usb_flash_disc_ejected(event.paths.first().expect("Should be at least one path")),
//                     _=> {}
//                 }
//             },
//             Err(e) => println!("watch error: {:?}", e),
//         }
//     }