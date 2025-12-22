use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::library::{Audiobook, Song};

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
