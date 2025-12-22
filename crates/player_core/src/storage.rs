use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::audio::{AudioFile, AudioFormat};
use crate::library::{Audiobook, AudiobookId, Chapter, Library, Song, SongId};

pub fn player_root() -> PathBuf {
    dirs::home_dir().expect("no home directory").join("Player")
}

pub fn library_root() -> PathBuf {
    player_root().join("Library")
}

pub fn manifest_path() -> PathBuf {
    player_root().join("lib.json")
}

#[derive(Debug, Clone)]
pub struct LibraryRoot(pub PathBuf);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativePath(pub PathBuf);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongEntry {
    pub path: RelativePath,
    pub format: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    #[serde(with = "duration_serde")]
    pub duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookEntry {
    pub path: RelativePath,
    pub format: String,
    pub title: String,
    pub author: Option<String>,
    pub chapters: Vec<ChapterEntry>,
    #[serde(with = "duration_serde")]
    pub total_duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterEntry {
    pub title: String,
    #[serde(with = "duration_serde")]
    pub start: Duration,
    #[serde(with = "duration_serde")]
    pub end: Duration,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LibraryManifest {
    pub songs: HashMap<u64, SongEntry>,
    pub audiobooks: HashMap<u64, AudiobookEntry>,
    pub next_song_id: u64,
    pub next_audiobook_id: u64,
}

pub trait PathResolver {
    fn song_path(&self, song: &Song) -> PathBuf;
    fn audiobook_path(&self, audiobook: &Audiobook) -> PathBuf;
}

pub trait Storage {
    type Error;

    fn load_manifest(&self) -> Result<LibraryManifest, Self::Error>;
    fn save_manifest(&self, manifest: &LibraryManifest) -> Result<(), Self::Error>;
    fn import_file(&self, source: &PathBuf, dest: &RelativePath) -> Result<(), Self::Error>;
    fn move_file(&self, from: &RelativePath, to: &RelativePath) -> Result<(), Self::Error>;
    fn delete_file(&self, path: &RelativePath) -> Result<(), Self::Error>;
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        duration.as_secs_f64().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

// ============================================================================
// Storage Error
// ============================================================================

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        StorageError::Io(e)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        StorageError::Json(e)
    }
}

// ============================================================================
// Conversions between Library and LibraryManifest
// ============================================================================

impl LibraryManifest {
    /// Convert a Library into a LibraryManifest for serialization.
    pub fn from_library(library: &Library) -> Self {
        let songs = library
            .songs
            .iter()
            .map(|(id, song)| {
                let entry = SongEntry {
                    path: RelativePath(song.file.path.clone()),
                    format: format_to_string(song.file.format),
                    title: song.title.clone(),
                    artist: song.artist.clone(),
                    album: song.album.clone(),
                    track_number: song.track_number,
                    duration: song.duration,
                };
                (id.0, entry)
            })
            .collect();

        let audiobooks = library
            .audiobooks
            .iter()
            .map(|(id, audiobook)| {
                let entry = AudiobookEntry {
                    path: RelativePath(audiobook.file.path.clone()),
                    format: format_to_string(audiobook.file.format),
                    title: audiobook.title.clone(),
                    author: audiobook.author.clone(),
                    chapters: audiobook
                        .chapters
                        .iter()
                        .map(|ch| ChapterEntry {
                            title: ch.title.clone(),
                            start: ch.start,
                            end: ch.end,
                        })
                        .collect(),
                    total_duration: audiobook.total_duration,
                };
                (id.0, entry)
            })
            .collect();

        let next_song_id = library.songs.keys().map(|id| id.0).max().unwrap_or(0) + 1;
        let next_audiobook_id = library.audiobooks.keys().map(|id| id.0).max().unwrap_or(0) + 1;

        LibraryManifest {
            songs,
            audiobooks,
            next_song_id,
            next_audiobook_id,
        }
    }

    /// Convert a LibraryManifest into a Library.
    pub fn into_library(self) -> Library {
        let songs = self
            .songs
            .into_iter()
            .map(|(id, entry)| {
                let song = Song {
                    id: SongId(id),
                    file: AudioFile {
                        path: entry.path.0,
                        format: format_from_string(&entry.format),
                    },
                    title: entry.title,
                    artist: entry.artist,
                    album: entry.album,
                    track_number: entry.track_number,
                    duration: entry.duration,
                };
                (SongId(id), song)
            })
            .collect();

        let audiobooks = self
            .audiobooks
            .into_iter()
            .map(|(id, entry)| {
                let audiobook = Audiobook {
                    id: AudiobookId(id),
                    file: AudioFile {
                        path: entry.path.0,
                        format: format_from_string(&entry.format),
                    },
                    title: entry.title,
                    author: entry.author,
                    chapters: entry
                        .chapters
                        .into_iter()
                        .map(|ch| Chapter {
                            title: ch.title,
                            start: ch.start,
                            end: ch.end,
                        })
                        .collect(),
                    total_duration: entry.total_duration,
                };
                (AudiobookId(id), audiobook)
            })
            .collect();

        Library { songs, audiobooks }
    }
}

fn format_to_string(format: AudioFormat) -> String {
    match format {
        AudioFormat::Mp3 => "mp3".to_string(),
        AudioFormat::M4b => "m4b".to_string(),
    }
}

fn format_from_string(s: &str) -> AudioFormat {
    match s {
        "mp3" => AudioFormat::Mp3,
        "m4b" => AudioFormat::M4b,
        _ => AudioFormat::Mp3, // Default fallback
    }
}

// ============================================================================
// Save / Load functions
// ============================================================================

/// Save a Library to the manifest file.
pub fn save_library(library: &Library) -> Result<(), StorageError> {
    let manifest = LibraryManifest::from_library(library);
    let path = manifest_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&path, json)?;

    Ok(())
}

/// Load a Library from the manifest file.
/// Returns an empty Library if the file doesn't exist.
pub fn load_library() -> Result<Library, StorageError> {
    let path = manifest_path();

    if !path.exists() {
        return Ok(Library::default());
    }

    let json = fs::read_to_string(&path)?;
    let manifest: LibraryManifest = serde_json::from_str(&json)?;

    Ok(manifest.into_library())
}
