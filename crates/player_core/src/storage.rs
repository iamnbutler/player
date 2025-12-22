use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::audio::{AudioFile, AudioFormat};
use crate::library::{Audiobook, AudiobookId, Chapter, Library, Song, SongId};

// ============================================================================
// Directory paths
// ============================================================================

pub fn player_root() -> PathBuf {
    dirs::home_dir().expect("no home directory").join("Player")
}

/// Where the library manifest is stored (JSONL format)
pub fn manifest_path() -> PathBuf {
    player_root().join("library.jsonl")
}

/// Where library audio files are stored (organized by artist/album)
pub fn music_path() -> PathBuf {
    player_root().join("Music")
}

/// Where users drop files to be imported
pub fn import_path() -> PathBuf {
    player_root().join("Import")
}

/// Where original files are moved after successful import
pub fn imported_path() -> PathBuf {
    player_root().join("Imported")
}

/// Where problematic files are moved when import fails
pub fn problem_path() -> PathBuf {
    player_root().join("Problem")
}

/// Ensure all required directories exist
pub fn ensure_directories() -> Result<(), StorageError> {
    fs::create_dir_all(player_root())?;
    fs::create_dir_all(music_path())?;
    fs::create_dir_all(import_path())?;
    fs::create_dir_all(imported_path())?;
    fs::create_dir_all(problem_path())?;
    Ok(())
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

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "IO error: {}", e),
            StorageError::Json(e) => write!(f, "JSON error: {}", e),
        }
    }
}

impl std::error::Error for StorageError {}

// ============================================================================
// JSONL Entry types (one per line in the manifest)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LibraryEntry {
    #[serde(rename = "song")]
    Song(SongEntry),
    #[serde(rename = "audiobook")]
    Audiobook(AudiobookEntry),
    #[serde(rename = "meta")]
    Meta(LibraryMeta),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryMeta {
    pub next_song_id: u64,
    pub next_audiobook_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongEntry {
    pub id: u64,
    pub path: PathBuf,
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
    pub id: u64,
    pub path: PathBuf,
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
// Conversions
// ============================================================================

impl SongEntry {
    pub fn from_song(song: &Song) -> Self {
        SongEntry {
            id: song.id.0,
            path: song.file.path.clone(),
            format: format_to_string(song.file.format),
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: song.album.clone(),
            track_number: song.track_number,
            duration: song.duration,
        }
    }

    pub fn into_song(self) -> Song {
        if self.duration.is_zero() {
            panic!(
                "Song has zero duration: id={}, path={:?}, title={:?}",
                self.id, self.path, self.title
            );
        }
        if self.duration.as_secs() > 24 * 60 * 60 {
            panic!(
                "Song has unreasonable duration (>24h, likely ms-as-seconds bug): id={}, path={:?}, title={:?}, duration={:?}",
                self.id, self.path, self.title, self.duration
            );
        }
        Song {
            id: SongId(self.id),
            file: AudioFile {
                path: self.path,
                format: format_from_string(&self.format),
            },
            title: self.title,
            artist: self.artist,
            album: self.album,
            track_number: self.track_number,
            duration: self.duration,
        }
    }
}

impl AudiobookEntry {
    pub fn from_audiobook(audiobook: &Audiobook) -> Self {
        AudiobookEntry {
            id: audiobook.id.0,
            path: audiobook.file.path.clone(),
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
        }
    }

    pub fn into_audiobook(self) -> Audiobook {
        Audiobook {
            id: AudiobookId(self.id),
            file: AudioFile {
                path: self.path,
                format: format_from_string(&self.format),
            },
            title: self.title,
            author: self.author,
            chapters: self
                .chapters
                .into_iter()
                .map(|ch| Chapter {
                    title: ch.title,
                    start: ch.start,
                    end: ch.end,
                })
                .collect(),
            total_duration: self.total_duration,
        }
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
        _ => AudioFormat::Mp3,
    }
}

// ============================================================================
// Streaming Save (JSONL format - one entry per line)
// ============================================================================

/// Save a Library to the manifest file in JSONL format.
/// Each line is a separate JSON object, making it resilient to partial corruption.
pub fn save_library(library: &Library) -> Result<(), StorageError> {
    ensure_directories()?;

    let path = manifest_path();
    let temp_path = path.with_extension("jsonl.tmp");

    let file = File::create(&temp_path)?;
    let mut writer = BufWriter::new(file);

    // Write metadata first
    let meta = LibraryEntry::Meta(LibraryMeta {
        next_song_id: library.songs.keys().map(|id| id.0).max().unwrap_or(0) + 1,
        next_audiobook_id: library.audiobooks.keys().map(|id| id.0).max().unwrap_or(0) + 1,
    });
    writeln!(writer, "{}", serde_json::to_string(&meta)?)?;

    // Write each song as a separate line
    for song in library.songs.values() {
        let entry = LibraryEntry::Song(SongEntry::from_song(song));
        writeln!(writer, "{}", serde_json::to_string(&entry)?)?;
    }

    // Write each audiobook as a separate line
    for audiobook in library.audiobooks.values() {
        let entry = LibraryEntry::Audiobook(AudiobookEntry::from_audiobook(audiobook));
        writeln!(writer, "{}", serde_json::to_string(&entry)?)?;
    }

    writer.flush()?;
    drop(writer);

    // Atomic rename
    fs::rename(&temp_path, &path)?;

    Ok(())
}

// ============================================================================
// Streaming Load (JSONL format)
// ============================================================================

/// Result of loading a single entry from the library
#[derive(Debug)]
pub enum LoadedEntry {
    Song(Song),
    Audiobook(Audiobook),
    Meta(LibraryMeta),
    /// Line was corrupted/invalid but we can continue
    Skipped {
        line_number: usize,
        error: String,
    },
}

/// Iterator that streams entries from the library file
pub struct LibraryReader {
    reader: BufReader<File>,
    line_number: usize,
    line_buffer: String,
}

impl LibraryReader {
    /// Open the library file for streaming reads
    pub fn open() -> Result<Option<Self>, StorageError> {
        let path = manifest_path();

        if !path.exists() {
            return Ok(None);
        }

        let file = File::open(&path)?;
        Ok(Some(LibraryReader {
            reader: BufReader::new(file),
            line_number: 0,
            line_buffer: String::new(),
        }))
    }
}

impl Iterator for LibraryReader {
    type Item = LoadedEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.line_buffer.clear();

        match self.reader.read_line(&mut self.line_buffer) {
            Ok(0) => None, // EOF
            Ok(_) => {
                self.line_number += 1;
                let line = self.line_buffer.trim();

                if line.is_empty() {
                    // Skip empty lines, try next
                    return self.next();
                }

                match serde_json::from_str::<LibraryEntry>(line) {
                    Ok(LibraryEntry::Song(entry)) => Some(LoadedEntry::Song(entry.into_song())),
                    Ok(LibraryEntry::Audiobook(entry)) => {
                        Some(LoadedEntry::Audiobook(entry.into_audiobook()))
                    }
                    Ok(LibraryEntry::Meta(meta)) => Some(LoadedEntry::Meta(meta)),
                    Err(e) => Some(LoadedEntry::Skipped {
                        line_number: self.line_number,
                        error: e.to_string(),
                    }),
                }
            }
            Err(e) => Some(LoadedEntry::Skipped {
                line_number: self.line_number,
                error: e.to_string(),
            }),
        }
    }
}

/// Load the entire library at once (convenience function)
pub fn load_library() -> Result<Library, StorageError> {
    let mut library = Library::default();

    if let Some(reader) = LibraryReader::open()? {
        for entry in reader {
            match entry {
                LoadedEntry::Song(song) => {
                    library.songs.insert(song.id, song);
                }
                LoadedEntry::Audiobook(audiobook) => {
                    library.audiobooks.insert(audiobook.id, audiobook);
                }
                LoadedEntry::Meta(_) => {
                    // Metadata is informational, we recalculate IDs as needed
                }
                LoadedEntry::Skipped { line_number, error } => {
                    eprintln!("Warning: Skipped corrupted line {}: {}", line_number, error);
                }
            }
        }
    }

    Ok(library)
}

// ============================================================================
// Legacy compatibility (keep old paths working during migration)
// ============================================================================

pub fn library_root() -> PathBuf {
    player_root().join("Library")
}

// ============================================================================
// Traits for abstraction (future use)
// ============================================================================

pub trait Storage {
    type Error;

    fn load_entries(&self) -> Result<Box<dyn Iterator<Item = LoadedEntry>>, Self::Error>;
    fn save_library(&self, library: &Library) -> Result<(), Self::Error>;
}
