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
    next_song_id: u64,
    next_audiobook_id: u64,
}

impl Library {
    /// Create a new empty library
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the next available song ID and increment the counter
    pub fn next_song_id(&mut self) -> SongId {
        let id = SongId(self.next_song_id);
        self.next_song_id += 1;
        id
    }

    /// Get the next available audiobook ID and increment the counter
    pub fn next_audiobook_id(&mut self) -> AudiobookId {
        let id = AudiobookId(self.next_audiobook_id);
        self.next_audiobook_id += 1;
        id
    }

    /// Add a song to the library
    pub fn add_song(&mut self, song: Song) {
        // Update next_song_id if necessary
        if song.id.0 >= self.next_song_id {
            self.next_song_id = song.id.0 + 1;
        }
        self.songs.insert(song.id, song);
    }

    /// Add an audiobook to the library
    pub fn add_audiobook(&mut self, audiobook: Audiobook) {
        // Update next_audiobook_id if necessary
        if audiobook.id.0 >= self.next_audiobook_id {
            self.next_audiobook_id = audiobook.id.0 + 1;
        }
        self.audiobooks.insert(audiobook.id, audiobook);
    }

    /// Check if the library is empty
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty() && self.audiobooks.is_empty()
    }

    /// Get total count of items in library
    pub fn len(&self) -> usize {
        self.songs.len() + self.audiobooks.len()
    }

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
