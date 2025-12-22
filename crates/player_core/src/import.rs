use std::path::Path;
use std::time::Duration;

use id3::TagLike;

use crate::audio::{AudioFile, AudioFormat};

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

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
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

pub trait MetadataReader {
    type Error;

    fn read(file: &AudioFile) -> Result<Metadata, Self::Error>;
}

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp3" => Some(AudioFormat::Mp3),
            "m4b" => Some(AudioFormat::M4b),
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }
}

pub struct Mp3MetadataReader;

impl MetadataReader for Mp3MetadataReader {
    type Error = ImportError;

    fn read(file: &AudioFile) -> Result<Metadata, Self::Error> {
        let tag = id3::Tag::read_from_path(&file.path)?;

        Ok(Metadata {
            title: tag.title().map(String::from),
            artist: tag.artist().map(String::from),
            album: tag.album().map(String::from),
            track_number: tag.track(),
            duration: tag.duration().map(|secs| Duration::from_secs(secs as u64)),
            chapters: Vec::new(),
        })
    }
}

pub fn import_file(path: impl AsRef<Path>) -> Result<ImportedFile, ImportError> {
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
