use std::path::PathBuf;

use crate::{FileSubscriber, FileSubscriberError};

pub struct VideoPlayer {
    _video_player_task: tokio::task::JoinHandle<()>,
    player_ctrl_tx: std::sync::mpsc::Sender<VideoPlayerCommand>,
}

#[derive(Debug)]
enum VideoPlayerCommand {
    Play(PathBuf),
    Stop(tokio::sync::oneshot::Sender<()>),
}


impl FileSubscriber for VideoPlayer {
    async fn on_file_about_to_be_deleted(&self) -> Result<(), FileSubscriberError> {
        tracing::info!("'on_file_about_to_be_deleted'");
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        let _ = self.player_ctrl_tx.send(VideoPlayerCommand::Stop(stop_tx));
        let _ = stop_rx.await;
        Ok(())
    }

    async fn on_new_file_available(&self, file_path: &std::path::Path) -> Result<(), FileSubscriberError> {
        tracing::info!("'on_new_file_available' {file_path:?}");
        let _ = self.player_ctrl_tx.send(VideoPlayerCommand::Play(file_path.to_path_buf()));
        Ok(())
    }
}

impl VideoPlayer {
    pub async fn run(looping: bool) -> Self {
        let (player_ctrl_tx, player_ctrl_rx) = std::sync::mpsc::channel();
        let player_ctrl_tx_shared = player_ctrl_tx.clone();
        
        let _video_player_task = tokio::task::spawn_blocking(move || {
            let vlc_instance = vlc::Instance::new().expect("Failed to create VLC instance");
            let player = vlc::MediaPlayer::new(&vlc_instance).expect("Failed to create MediaPlayer");

            loop {
                match player_ctrl_rx.recv()  {
                    Ok(VideoPlayerCommand::Play(path_buf)) => {
                        tracing::info!("VLC playing {path_buf:?}");
                        if let Some(media) = vlc::Media::new_path(&vlc_instance, &path_buf) {
                            let path_buf_sharde = path_buf.clone();
                            let player_ctrl_tx_shared = player_ctrl_tx_shared.clone();
                            if media.event_manager().attach(vlc::EventType::MediaStateChanged, move |event, _| {
                                if let vlc::Event::MediaStateChanged(state) = event {
                                    if looping && (state == vlc::State::Ended || state == vlc::State::Error) {
                                        tracing::debug!("Looping");
                                        let _ = player_ctrl_tx_shared.send(VideoPlayerCommand::Play(path_buf_sharde.clone()));
                                    }
                                }
                            }).is_err() {
                                tracing::error!("Failed to set VLC event manager");
                            }
                            
                            
                            player.set_media(&media);
                            if player.play().is_err() {
                                tracing::warn!("Video Player could not play {path_buf:?}.");
                            }
                        } else {
                            tracing::warn!("Video {path_buf:?} not found by VLC");
                        }
                    },
                    Ok(VideoPlayerCommand::Stop(stop_feedback_tx)) => {
                        tracing::info!("VLC ptopping playback");
                        player.stop();
                        tracing::debug!("VLC state after stop: {:?}", player.state());

                        if stop_feedback_tx.send(()).is_err() {
                            tracing::warn!("Video Player stop failed send feedback.");
                        }
                    },
                    Err(_) => {
                        tracing::info!("Video Player shutting down.");
                        break;
                    }
                }
            }
        });
        
        Self { _video_player_task, player_ctrl_tx }
    }
}



