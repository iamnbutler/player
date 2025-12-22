use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    M4b,
}

#[derive(Debug, Clone)]
pub struct AudioFile {
    pub path: PathBuf,
    pub format: AudioFormat,
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub trait Decoder {
    type Error;

    fn open(file: &AudioFile) -> Result<Self, Self::Error>
    where
        Self: Sized;
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn duration(&self) -> Option<Duration>;
    fn seek(&mut self, position: Duration) -> Result<(), Self::Error>;
    fn next_frame(&mut self) -> Option<Result<AudioFrame, Self::Error>>;
}
