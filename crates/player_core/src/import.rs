use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use id3::TagLike;

use crate::audio::{AudioFile, AudioFormat};
use crate::library::{Library, Song, SongId};
use crate::storage::{import_path, imported_path, music_path};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug)]
pub enum ImportError {
    UnknownFormat,
    IoError(std::io::Error),
    Id3Error(id3::Error),
}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::IoError(e)
    }
}

impl From<id3::Error> for ImportError {
    fn from(e: id3::Error) -> Self {
        ImportError::Id3Error(e)
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::UnknownFormat => write!(f, "Unknown audio format"),
            ImportError::IoError(e) => write!(f, "IO error: {}", e),
            ImportError::Id3Error(e) => write!(f, "ID3 error: {}", e),
        }
    }
}

impl std::error::Error for ImportError {}

// ============================================================================
// Metadata Types
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub duration: Option<Duration>,
    pub chapters: Vec<ChapterMeta>,
}

#[derive(Debug, Clone)]
pub struct ChapterMeta {
    pub title: Option<String>,
    pub start: Duration,
    pub end: Duration,
}

#[derive(Debug, Clone)]
pub struct ImportedFile {
    pub file: AudioFile,
    pub metadata: Metadata,
}

// ============================================================================
// Result of importing a file into the library
// ============================================================================

#[derive(Debug)]
pub struct ImportResult {
    pub song: Song,
    pub original_path: PathBuf,
    pub library_path: PathBuf,
    pub archived_path: PathBuf,
}

// ============================================================================
// Metadata Reader Trait
// ============================================================================

pub trait MetadataReader {
    type Error;

    fn read(file: &AudioFile) -> Result<Metadata, Self::Error>;
}

// ============================================================================
// Audio Format Detection
// ============================================================================

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp3" => Some(AudioFormat::Mp3),
            "m4b" => Some(AudioFormat::M4b),
            "m4a" => Some(AudioFormat::M4b), // Treat m4a as m4b for now
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::M4b => "m4b",
        }
    }
}

// ============================================================================
// MP3 Metadata Reader
// ============================================================================

pub struct Mp3MetadataReader;

impl MetadataReader for Mp3MetadataReader {
    type Error = ImportError;

    fn read(file: &AudioFile) -> Result<Metadata, Self::Error> {
        let tag = id3::Tag::read_from_path(&file.path)?;

        Ok(Metadata {
            title: tag.title().map(String::from),
            artist: tag.artist().map(String::from),
            album_artist: tag.album_artist().map(String::from),
            album: tag.album().map(String::from),
            track_number: tag.track(),
            duration: tag.duration().map(|secs| Duration::from_secs(secs as u64)),
            chapters: Vec::new(),
        })
    }
}

// ============================================================================
// Basic Import Functions
// ============================================================================

/// Read metadata from an audio file without importing it
pub fn read_metadata(path: impl AsRef<Path>) -> Result<ImportedFile, ImportError> {
    let path = path.as_ref();
    let format = AudioFormat::from_path(path).ok_or(ImportError::UnknownFormat)?;

    let file = AudioFile {
        path: path.to_path_buf(),
        format,
    };

    let metadata = match format {
        AudioFormat::Mp3 => Mp3MetadataReader::read(&file)?,
        AudioFormat::M4b => Metadata::default(),
    };

    Ok(ImportedFile { file, metadata })
}

/// Recursively scan a directory for audio files and read their metadata.
/// Does NOT copy or move files - just reads them in place.
pub fn scan_directory(path: impl AsRef<Path>) -> Result<Vec<ImportedFile>, ImportError> {
    let path = path.as_ref();
    let mut imported = Vec::new();
    let mut paths_to_scan: Vec<PathBuf> = vec![path.to_path_buf()];

    while let Some(current_path) = paths_to_scan.pop() {
        let entries = match fs::read_dir(&current_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                paths_to_scan.push(entry_path);
            } else if entry_path.is_file() {
                // Try to read metadata, skip if it fails
                if let Ok(imported_file) = read_metadata(&entry_path) {
                    imported.push(imported_file);
                }
            }
        }
    }

    Ok(imported)
}

// ============================================================================
// Full Import Workflow
// ============================================================================

/// Generate a safe filename from a string (remove/replace invalid characters)
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Generate the library path for a song based on its metadata
/// Format: ~/Player/Music/Artist/Album/TrackNum - Title.ext
fn generate_library_path(metadata: &Metadata, format: AudioFormat) -> PathBuf {
    let artist = metadata
        .artist
        .as_ref()
        .or(metadata.album_artist.as_ref())
        .map(|s| sanitize_filename(s))
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = metadata
        .album
        .as_ref()
        .map(|s| sanitize_filename(s))
        .unwrap_or_else(|| "Unknown Album".to_string());

    let title = metadata
        .title
        .as_ref()
        .map(|s| sanitize_filename(s))
        .unwrap_or_else(|| "Unknown Title".to_string());

    let filename = match metadata.track_number {
        Some(num) => format!("{:02} - {}.{}", num, title, format.extension()),
        None => format!("{}.{}", title, format.extension()),
    };

    music_path().join(&artist).join(&album).join(&filename)
}

/// Generate the archived path for a file, preserving its relative structure from Import/
fn generate_archived_path(original_path: &Path) -> PathBuf {
    let import_dir = import_path();

    // Try to preserve relative path structure
    let relative = original_path
        .strip_prefix(&import_dir)
        .unwrap_or(original_path);

    imported_path().join(relative)
}

/// Import a single file into the library:
/// 1. Read metadata
/// 2. Copy to ~/Player/Music/Artist/Album/
/// 3. Move original to ~/Player/Imported/
/// 4. Return the new Song
pub fn import_file_to_library(
    source_path: impl AsRef<Path>,
    next_id: u64,
) -> Result<ImportResult, ImportError> {
    let source_path = source_path.as_ref();

    // Read metadata from source
    let imported = read_metadata(source_path)?;

    // Generate destination paths
    let library_path = generate_library_path(&imported.metadata, imported.file.format);
    let archived_path = generate_archived_path(source_path);

    // Create destination directories
    if let Some(parent) = library_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = archived_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Copy file to library
    fs::copy(source_path, &library_path)?;

    // Move original to archived
    fs::rename(source_path, &archived_path)?;

    // Create the song with the new library path
    let song = Song {
        id: SongId(next_id),
        file: AudioFile {
            path: library_path.clone(),
            format: imported.file.format,
        },
        title: imported
            .metadata
            .title
            .unwrap_or_else(|| "Unknown Title".to_string()),
        artist: imported.metadata.artist.or(imported.metadata.album_artist),
        album: imported.metadata.album,
        track_number: imported.metadata.track_number,
        duration: imported.metadata.duration.unwrap_or(Duration::ZERO),
    };

    Ok(ImportResult {
        song,
        original_path: source_path.to_path_buf(),
        library_path,
        archived_path,
    })
}

/// Scan the Import directory and import all new files
pub fn import_all_pending(library: &mut Library) -> Vec<Result<ImportResult, ImportError>> {
    let import_dir = import_path();
    let mut results = Vec::new();

    // Ensure import directory exists
    if !import_dir.exists() {
        if let Err(e) = fs::create_dir_all(&import_dir) {
            results.push(Err(ImportError::IoError(e)));
            return results;
        }
    }

    // Get next available song ID
    let mut next_id = library.songs.keys().map(|id| id.0).max().unwrap_or(0) + 1;

    // Scan for files
    let files = match scan_directory(&import_dir) {
        Ok(files) => files,
        Err(e) => {
            results.push(Err(e));
            return results;
        }
    };

    // Import each file
    for file in files {
        let source_path = file.file.path.clone();

        match import_file_to_library(&source_path, next_id) {
            Ok(result) => {
                library.songs.insert(result.song.id, result.song.clone());
                next_id += 1;
                results.push(Ok(result));
            }
            Err(e) => {
                results.push(Err(e));
            }
        }
    }

    // Clean up empty directories in Import folder
    cleanup_empty_directories(&import_dir);

    results
}

/// Recursively remove empty directories from the given path
fn cleanup_empty_directories(path: &Path) {
    if !path.is_dir() {
        return;
    }

    // First, recurse into subdirectories
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                cleanup_empty_directories(&entry_path);
            }
        }
    }

    // Then try to remove this directory if it's empty
    // (This will fail silently if the directory is not empty, which is fine)
    let _ = fs::remove_dir(path);
}
