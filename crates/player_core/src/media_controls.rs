use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

#[derive(Debug, Clone)]
pub enum MediaKeyEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SeekForward,
    SeekBackward,
    SetPosition(Duration),
}

pub struct MediaControlsHandler {
    controls: MediaControls,
    receiver: Receiver<MediaKeyEvent>,
}

#[derive(Debug)]
pub enum MediaControlsError {
    InitFailed(String),
    AttachFailed(String),
    UpdateFailed(String),
}

impl std::fmt::Display for MediaControlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaControlsError::InitFailed(e) => {
                write!(f, "Failed to initialize media controls: {}", e)
            }
            MediaControlsError::AttachFailed(e) => {
                write!(f, "Failed to attach media controls: {}", e)
            }
            MediaControlsError::UpdateFailed(e) => {
                write!(f, "Failed to update media controls: {}", e)
            }
        }
    }
}

impl std::error::Error for MediaControlsError {}

impl MediaControlsHandler {
    pub fn new() -> Result<Self, MediaControlsError> {
        let config = PlatformConfig {
            dbus_name: "player",
            display_name: "Player",
            hwnd: None,
        };

        let mut controls = MediaControls::new(config)
            .map_err(|e| MediaControlsError::InitFailed(e.to_string()))?;

        let (sender, receiver) = mpsc::channel::<MediaKeyEvent>();

        Self::attach_handler(&mut controls, sender)?;

        Ok(Self { controls, receiver })
    }

    fn attach_handler(
        controls: &mut MediaControls,
        sender: Sender<MediaKeyEvent>,
    ) -> Result<(), MediaControlsError> {
        controls
            .attach(move |event: MediaControlEvent| {
                let media_event = match event {
                    MediaControlEvent::Play => Some(MediaKeyEvent::Play),
                    MediaControlEvent::Pause => Some(MediaKeyEvent::Pause),
                    MediaControlEvent::Toggle => Some(MediaKeyEvent::Toggle),
                    MediaControlEvent::Next => Some(MediaKeyEvent::Next),
                    MediaControlEvent::Previous => Some(MediaKeyEvent::Previous),
                    MediaControlEvent::Stop => Some(MediaKeyEvent::Stop),
                    MediaControlEvent::Seek(souvlaki::SeekDirection::Forward) => {
                        Some(MediaKeyEvent::SeekForward)
                    }
                    MediaControlEvent::Seek(souvlaki::SeekDirection::Backward) => {
                        Some(MediaKeyEvent::SeekBackward)
                    }
                    MediaControlEvent::SetPosition(pos) => Some(MediaKeyEvent::SetPosition(pos.0)),
                    _ => None,
                };

                if let Some(event) = media_event {
                    let _ = sender.send(event);
                }
            })
            .map_err(|e| MediaControlsError::AttachFailed(e.to_string()))
    }

    pub fn try_recv(&self) -> Option<MediaKeyEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn poll_events(&self) -> Vec<MediaKeyEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn set_metadata(
        &mut self,
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        duration: Option<Duration>,
    ) -> Result<(), MediaControlsError> {
        self.controls
            .set_metadata(MediaMetadata {
                title,
                artist,
                album,
                duration,
                ..Default::default()
            })
            .map_err(|e| MediaControlsError::UpdateFailed(e.to_string()))
    }

    pub fn set_playback_playing(
        &mut self,
        position: Option<Duration>,
    ) -> Result<(), MediaControlsError> {
        let progress = position.map(MediaPosition);
        self.controls
            .set_playback(MediaPlayback::Playing { progress })
            .map_err(|e| MediaControlsError::UpdateFailed(e.to_string()))
    }

    pub fn set_playback_paused(
        &mut self,
        position: Option<Duration>,
    ) -> Result<(), MediaControlsError> {
        let progress = position.map(MediaPosition);
        self.controls
            .set_playback(MediaPlayback::Paused { progress })
            .map_err(|e| MediaControlsError::UpdateFailed(e.to_string()))
    }

    pub fn set_playback_stopped(&mut self) -> Result<(), MediaControlsError> {
        self.controls
            .set_playback(MediaPlayback::Stopped)
            .map_err(|e| MediaControlsError::UpdateFailed(e.to_string()))
    }

    pub fn update_from_song(
        &mut self,
        title: &str,
        artist: Option<&str>,
        album: Option<&str>,
        duration: Duration,
        playing: bool,
        position: Duration,
    ) -> Result<(), MediaControlsError> {
        self.set_metadata(Some(title), artist, album, Some(duration))?;

        if playing {
            self.set_playback_playing(Some(position))?;
        } else {
            self.set_playback_paused(Some(position))?;
        }

        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), MediaControlsError> {
        self.set_metadata(None, None, None, None)?;
        self.set_playback_stopped()?;
        Ok(())
    }
}
