use std::collections::HashMap;
use std::time::Duration;

use crate::audio::AudioFile;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    Artist,
    Album,
    Title,
}

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

impl Library {
    /// Returns a sorted list of songs based on the given sort order.
    pub fn list(&self, sort_order: SortOrder) -> Vec<Song> {
        let mut songs: Vec<Song> = self.songs.values().cloned().collect();

        match sort_order {
            SortOrder::Artist => {
                songs.sort_by(|a, b| {
                    let artist_a = a.artist.as_deref().unwrap_or("");
                    let artist_b = b.artist.as_deref().unwrap_or("");
                    artist_a
                        .cmp(artist_b)
                        .then_with(|| {
                            let album_a = a.album.as_deref().unwrap_or("");
                            let album_b = b.album.as_deref().unwrap_or("");
                            album_a.cmp(album_b)
                        })
                        .then_with(|| a.track_number.cmp(&b.track_number))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            SortOrder::Album => {
                songs.sort_by(|a, b| {
                    let album_a = a.album.as_deref().unwrap_or("");
                    let album_b = b.album.as_deref().unwrap_or("");
                    album_a
                        .cmp(album_b)
                        .then_with(|| a.track_number.cmp(&b.track_number))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            SortOrder::Title => {
                songs.sort_by(|a, b| a.title.cmp(&b.title));
            }
        }

        songs
    }
}
