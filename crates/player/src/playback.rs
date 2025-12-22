use std::time::Duration;

use crate::library::MediaItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug)]
pub struct NowPlaying {
    pub item: Option<MediaItem>,
    pub state: PlaybackState,
    pub position: Duration,
    pub volume: f32,
}

impl Default for NowPlaying {
    fn default() -> Self {
        Self {
            item: None,
            state: PlaybackState::Stopped,
            position: Duration::ZERO,
            volume: 1.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Queue {
    pub items: Vec<MediaItem>,
    pub current: Option<usize>,
}
