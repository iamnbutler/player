use std::io::BufReader;
use std::time::{Duration, Instant};

use gpui::{Context, EventEmitter};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

use crate::library::Song;
use crate::playback::PlaybackState;

pub struct AudioPlayer {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sink: Option<Sink>,
    current_song: Option<Song>,
    state: PlaybackState,
    volume: f32,
    playback_started_at: Option<Instant>,
    paused_position: Duration,
}

pub enum AudioPlayerEvent {
    StateChanged(PlaybackState),
    SongChanged(Option<Song>),
    PlaybackFinished,
}

impl EventEmitter<AudioPlayerEvent> for AudioPlayer {}

impl AudioPlayer {
    pub fn new(_cx: &mut Context<Self>) -> Result<Self, AudioPlayerError> {
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| AudioPlayerError::OutputStreamError(e.to_string()))?;

        Ok(Self {
            _stream: stream,
            stream_handle,
            sink: None,
            current_song: None,
            state: PlaybackState::Stopped,
            volume: 1.0,
            playback_started_at: None,
            paused_position: Duration::ZERO,
        })
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn current_song(&self) -> Option<&Song> {
        self.current_song.as_ref()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn position(&self) -> Duration {
        match self.state {
            PlaybackState::Playing => {
                if let Some(started_at) = self.playback_started_at {
                    self.paused_position + started_at.elapsed()
                } else {
                    self.paused_position
                }
            }
            PlaybackState::Paused | PlaybackState::Stopped => self.paused_position,
        }
    }

    pub fn play_song(
        &mut self,
        song: Song,
        cx: &mut Context<Self>,
    ) -> Result<(), AudioPlayerError> {
        self.stop_internal();

        let path = &song.file.path;
        let file =
            std::fs::File::open(path).map_err(|e| AudioPlayerError::FileError(e.to_string()))?;
        let reader = BufReader::new(file);

        let source =
            Decoder::new(reader).map_err(|e| AudioPlayerError::DecodeError(e.to_string()))?;

        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| AudioPlayerError::SinkError(e.to_string()))?;

        sink.set_volume(self.volume);
        sink.append(source);

        self.sink = Some(sink);
        self.current_song = Some(song.clone());
        self.state = PlaybackState::Playing;
        self.playback_started_at = Some(Instant::now());
        self.paused_position = Duration::ZERO;

        cx.emit(AudioPlayerEvent::SongChanged(Some(song)));
        cx.emit(AudioPlayerEvent::StateChanged(PlaybackState::Playing));
        cx.notify();

        Ok(())
    }

    pub fn play(&mut self, cx: &mut Context<Self>) {
        if let Some(sink) = &self.sink {
            if self.state == PlaybackState::Paused {
                sink.play();
                self.state = PlaybackState::Playing;
                self.playback_started_at = Some(Instant::now());
                cx.emit(AudioPlayerEvent::StateChanged(PlaybackState::Playing));
                cx.notify();
            }
        }
    }

    pub fn pause(&mut self, cx: &mut Context<Self>) {
        if let Some(sink) = &self.sink {
            if self.state == PlaybackState::Playing {
                sink.pause();
                self.paused_position = self.position();
                self.playback_started_at = None;
                self.state = PlaybackState::Paused;
                cx.emit(AudioPlayerEvent::StateChanged(PlaybackState::Paused));
                cx.notify();
            }
        }
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        match self.state {
            PlaybackState::Playing => self.pause(cx),
            PlaybackState::Paused => self.play(cx),
            PlaybackState::Stopped => {}
        }
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.stop_internal();
        self.current_song = None;
        self.state = PlaybackState::Stopped;
        cx.emit(AudioPlayerEvent::SongChanged(None));
        cx.emit(AudioPlayerEvent::StateChanged(PlaybackState::Stopped));
        cx.notify();
    }

    fn stop_internal(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.playback_started_at = None;
        self.paused_position = Duration::ZERO;
    }

    pub fn set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
        cx.notify();
    }

    pub fn seek_to(&mut self, position: Duration, cx: &mut Context<Self>) {
        if let Some(sink) = &self.sink {
            if sink.try_seek(position).is_ok() {
                self.paused_position = position;
                if self.state == PlaybackState::Playing {
                    self.playback_started_at = Some(Instant::now());
                }
                cx.notify();
            }
        }
    }

    pub fn seek_by(&mut self, delta: Duration, forward: bool, cx: &mut Context<Self>) {
        let current_position = self.position();
        let new_position = if forward {
            current_position.saturating_add(delta)
        } else {
            current_position.saturating_sub(delta)
        };
        self.seek_to(new_position, cx);
    }

    pub fn is_finished(&self) -> bool {
        self.sink.as_ref().is_some_and(|s| s.empty())
    }

    pub fn check_and_handle_finished(&mut self, cx: &mut Context<Self>) -> bool {
        if self.state == PlaybackState::Playing && self.is_finished() {
            self.stop_internal();
            self.state = PlaybackState::Stopped;
            cx.emit(AudioPlayerEvent::PlaybackFinished);
            cx.notify();
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioPlayerError {
    OutputStreamError(String),
    FileError(String),
    DecodeError(String),
    SinkError(String),
}

impl std::fmt::Display for AudioPlayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioPlayerError::OutputStreamError(e) => write!(f, "Output stream error: {}", e),
            AudioPlayerError::FileError(e) => write!(f, "File error: {}", e),
            AudioPlayerError::DecodeError(e) => write!(f, "Decode error: {}", e),
            AudioPlayerError::SinkError(e) => write!(f, "Sink error: {}", e),
        }
    }
}

impl std::error::Error for AudioPlayerError {}
