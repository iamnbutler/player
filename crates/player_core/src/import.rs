use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use id3::{Tag, TagLike};
use rayon::prelude::*;
use rodio::{Decoder, Source};

use crate::audio::{AudioFile, AudioFormat};
use crate::library::{Library, Song, SongId};
use crate::storage::{import_path, imported_path, music_path, problem_path};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug)]
pub enum ImportError {
    UnknownFormat,
    NoDuration(PathBuf),
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
            ImportError::NoDuration(path) => write!(f, "Could not determine duration: {:?}", path),
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
            duration: get_audio_duration(&file.path)
                .or_else(|| {
                    tag.duration()
                        .map(|millis| Duration::from_millis(millis as u64))
                })
                .or_else(|| {
                    let duration = calculate_duration_by_decoding(&file.path)?;
                    if write_duration_to_file(&file.path, duration).is_ok() {
                        eprintln!(
                            "Wrote calculated duration {:?} to {:?}",
                            duration, file.path
                        );
                    }
                    Some(duration)
                }),
            chapters: Vec::new(),
        })
    }
}

fn get_audio_duration(path: &Path) -> Option<Duration> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let decoder = Decoder::new(reader).ok()?;
    decoder.total_duration()
}

fn calculate_duration_by_decoding(path: &Path) -> Option<Duration> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let decoder = Decoder::new(reader).ok()?;

    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels() as u64;

    if sample_rate == 0 || channels == 0 {
        return None;
    }

    let total_samples: u64 = decoder.count() as u64;
    let duration_secs = total_samples as f64 / (sample_rate as f64 * channels as f64);
    let rounded_secs = duration_secs.round() as u64;

    Some(Duration::from_secs(rounded_secs))
}

fn write_duration_to_file(path: &Path, duration: Duration) -> Result<(), ImportError> {
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::new());
    let millis = duration.as_millis() as u32;
    tag.set_duration(millis);
    tag.write_to_path(path, id3::Version::Id3v24)?;
    Ok(())
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

fn generate_problem_path(original_path: &Path) -> PathBuf {
    let import_dir = import_path();

    // Try to preserve relative path structure
    let relative = original_path
        .strip_prefix(&import_dir)
        .unwrap_or(original_path);

    problem_path().join(relative)
}

/// Import a single file into the library:
/// 1. Read metadata
/// 2. Copy to ~/Player/Music/Artist/Album/
/// 3. Move original to ~/Player/Imported/
/// 4. Return the new Song
///
/// If duration cannot be determined, moves file to ~/Player/Problem/ and returns NoDuration error.
pub fn import_file_to_library(
    source_path: impl AsRef<Path>,
    next_id: u64,
) -> Result<ImportResult, ImportError> {
    let source_path = source_path.as_ref();

    // Read metadata from source
    let imported = read_metadata(source_path)?;

    // Check duration before we copy anything
    let duration = match imported.metadata.duration {
        Some(d) => d,
        None => {
            // Move to Problem folder
            let problem_dest = generate_problem_path(source_path);
            if let Some(parent) = problem_dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(source_path, &problem_dest)?;
            return Err(ImportError::NoDuration(problem_dest));
        }
    };

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
        duration,
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

#[derive(Debug, Clone)]
pub struct RepairResult {
    pub path: PathBuf,
    pub duration: Duration,
    pub moved_to: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RepairFailure {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct RepairProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: PathBuf,
}

/// Attempt to repair files in the Problem folder by calculating duration and writing it to ID3.
/// Successfully repaired files are moved back to the Import folder.
pub fn repair_problem_files() -> (Vec<RepairResult>, Vec<RepairFailure>) {
    repair_problem_files_with_progress(|_| {})
}

/// Attempt to repair files in the Problem folder with progress callback.
/// The callback receives progress info for each file being processed.
/// Uses parallel processing for CPU-bound decoding work.
pub fn repair_problem_files_with_progress<F>(
    on_progress: F,
) -> (Vec<RepairResult>, Vec<RepairFailure>)
where
    F: Fn(RepairProgress) + Send + Sync,
{
    let problem_dir = problem_path();

    if !problem_dir.exists() {
        return (Vec::new(), Vec::new());
    }

    let files = match scan_directory(&problem_dir) {
        Ok(files) => files,
        Err(e) => {
            return (
                Vec::new(),
                vec![RepairFailure {
                    path: problem_dir,
                    reason: format!("Failed to scan directory: {}", e),
                }],
            );
        }
    };

    let total = files.len();
    let processed = Arc::new(AtomicUsize::new(0));

    let results: Vec<Result<RepairResult, RepairFailure>> = files
        .into_par_iter()
        .map(|file| {
            let path = file.file.path.clone();
            let current = processed.fetch_add(1, Ordering::SeqCst) + 1;

            on_progress(RepairProgress {
                current,
                total,
                current_file: path.clone(),
            });

            let duration = calculate_duration_by_decoding(&path).ok_or_else(|| RepairFailure {
                path: path.clone(),
                reason: "Could not calculate duration by decoding".to_string(),
            })?;

            write_duration_to_file(&path, duration).map_err(|e| RepairFailure {
                path: path.clone(),
                reason: format!("Failed to write duration to file: {}", e),
            })?;

            let relative = path.strip_prefix(&problem_dir).unwrap_or(&path);
            let import_dest = import_path().join(relative);

            if let Some(parent) = import_dest.parent() {
                fs::create_dir_all(parent).map_err(|e| RepairFailure {
                    path: path.clone(),
                    reason: format!("Failed to create import directory: {}", e),
                })?;
            }

            fs::rename(&path, &import_dest).map_err(|e| RepairFailure {
                path: path.clone(),
                reason: format!("Failed to move to Import: {}", e),
            })?;

            eprintln!("Repaired {:?} with duration {:?}", import_dest, duration);
            Ok(RepairResult {
                path,
                duration,
                moved_to: import_dest,
            })
        })
        .collect();

    let mut successes = Vec::new();
    let mut failures = Vec::new();

    for result in results {
        match result {
            Ok(success) => successes.push(success),
            Err(failure) => failures.push(failure),
        }
    }

    cleanup_empty_directories(&problem_dir);

    (successes, failures)
}
