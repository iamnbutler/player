use std::collections::HashMap;
use std::time::Duration;

use crate::audio::AudioFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SongId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudiobookId(pub u64);

#[derive(Debug, Clone)]
pub struct Song {
    pub id: SongId,
    pub file: AudioFile,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct Audiobook {
    pub id: AudiobookId,
    pub file: AudioFile,
    pub title: String,
    pub author: Option<String>,
    pub chapters: Vec<Chapter>,
    pub total_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct Chapter {
    pub title: String,
    pub start: Duration,
    pub end: Duration,
}

#[derive(Debug, Clone)]
pub enum MediaItem {
    Song(Song),
    Audiobook(Audiobook),
}

#[derive(Debug, Default)]
pub struct Library {
    pub songs: HashMap<SongId, Song>,
    pub audiobooks: HashMap<AudiobookId, Audiobook>,
}
