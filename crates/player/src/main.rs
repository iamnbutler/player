use gpui::prelude::*;
use gpui::*;
use player_core::{
    ensure_directories, import_all_pending, import_path, save_library, AudioPlayer,
    AudioPlayerEvent, Library, LibraryReader, LoadedEntry, PlaybackState, Song,
};
use ui::{ListView, ListViewEvent};

struct Player {
    library: Entity<Library>,
    list_view: Entity<ListView>,
    audio_player: Entity<AudioPlayer>,
    _subscriptions: Vec<Subscription>,
}

impl Player {
    fn new(cx: &mut Context<Self>) -> Self {
        if let Err(e) = ensure_directories() {
            eprintln!("Failed to create directories: {}", e);
        }

        let library = cx.new(|_cx| Library::new());

        Self::stream_load_library(library.clone(), cx);

        let audio_player =
            cx.new(|cx| AudioPlayer::new(cx).expect("Failed to create audio player"));

        let list_view = cx.new(|cx| ListView::new(library.clone(), cx));

        let subscriptions = vec![
            cx.subscribe(&list_view, Self::handle_list_view_event),
            cx.subscribe(&audio_player, Self::handle_audio_player_event),
        ];

        Player {
            library,
            list_view,
            audio_player,
            _subscriptions: subscriptions,
        }
    }

    fn handle_list_view_event(
        &mut self,
        _list_view: Entity<ListView>,
        event: &ListViewEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ListViewEvent::SongSelected(_song) => {}
            ListViewEvent::SongDoubleClicked(song) => {
                self.play_song(song.clone(), cx);
            }
        }
    }

    fn handle_audio_player_event(
        &mut self,
        _audio_player: Entity<AudioPlayer>,
        event: &AudioPlayerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            AudioPlayerEvent::StateChanged(_state) => {
                cx.notify();
            }
            AudioPlayerEvent::SongChanged(song) => {
                let song_id = song.as_ref().map(|s| s.id);
                self.list_view.update(cx, |list_view, cx| {
                    list_view.set_playing_song(song_id, cx);
                });
                cx.notify();
            }
            AudioPlayerEvent::PlaybackFinished => {
                cx.notify();
            }
        }
    }

    fn play_song(&mut self, song: Song, cx: &mut Context<Self>) {
        self.audio_player.update(cx, |player, cx| {
            if let Err(e) = player.play_song(song, cx) {
                eprintln!("Failed to play song: {}", e);
            }
        });
    }

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        self.audio_player.update(cx, |player, cx| {
            player.toggle_playback(cx);
        });
    }

    fn stream_load_library(library: Entity<Library>, cx: &mut Context<Self>) {
        cx.spawn(async move |_this, cx| {
            let reader = match LibraryReader::open() {
                Ok(Some(reader)) => reader,
                Ok(None) => {
                    println!("No existing library found");
                    return;
                }
                Err(e) => {
                    eprintln!("Failed to open library: {}", e);
                    return;
                }
            };

            let mut song_count = 0;
            let mut batch: Vec<player_core::Song> = Vec::new();
            const BATCH_SIZE: usize = 100;

            for entry in reader {
                match entry {
                    LoadedEntry::Song(song) => {
                        batch.push(song);
                        song_count += 1;

                        if batch.len() >= BATCH_SIZE {
                            let songs_to_add = std::mem::take(&mut batch);
                            let _ = library.update(cx, |lib, cx| {
                                for song in songs_to_add {
                                    lib.add_song(song);
                                }
                                cx.notify();
                            });
                        }
                    }
                    LoadedEntry::Audiobook(audiobook) => {
                        let _ = library.update(cx, |lib, cx| {
                            lib.add_audiobook(audiobook);
                            cx.notify();
                        });
                    }
                    LoadedEntry::Meta(_) => {}
                    LoadedEntry::Skipped { line_number, error } => {
                        eprintln!("Warning: Skipped line {}: {}", line_number, error);
                    }
                }
            }

            if !batch.is_empty() {
                let _ = library.update(cx, |lib, cx| {
                    for song in batch {
                        lib.add_song(song);
                    }
                    cx.notify();
                });
            }

            println!("Loaded {} songs from library", song_count);
        })
        .detach();
    }

    fn scan_for_imports(&mut self, cx: &mut Context<Self>) {
        let library = self.library.clone();

        cx.spawn(async move |_this, cx| {
            println!("Scanning {} for new files...", import_path().display());

            let mut lib = Library::new();

            if let Ok(current_songs) =
                library.read_with(cx, |current_lib, _cx| current_lib.songs.clone())
            {
                lib.songs = current_songs;
            }

            if let Ok(current_audiobooks) =
                library.read_with(cx, |current_lib, _cx| current_lib.audiobooks.clone())
            {
                lib.audiobooks = current_audiobooks;
            }

            let results = import_all_pending(&mut lib);

            let success_count = results.iter().filter(|r| r.is_ok()).count();
            let error_count = results.iter().filter(|r| r.is_err()).count();

            if success_count > 0 {
                println!("Imported {} new files", success_count);

                if let Err(e) = save_library(&lib) {
                    eprintln!("Failed to save library: {}", e);
                }

                let new_songs = lib.songs;
                let _ = library.update(cx, |current_lib, cx| {
                    for (id, song) in new_songs {
                        if !current_lib.songs.contains_key(&id) {
                            current_lib.add_song(song);
                        }
                    }
                    cx.notify();
                });
            }

            if error_count > 0 {
                eprintln!("{} files failed to import", error_count);
                for result in &results {
                    if let Err(e) = result {
                        eprintln!("  - {}", e);
                    }
                }
            }

            if success_count == 0 && error_count == 0 {
                println!("No new files to import");
            }
        })
        .detach();
    }
}

impl Render for Player {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let song_count = self.library.read(cx).songs.len();
        let audio_player = self.audio_player.read(cx);
        let playback_state = audio_player.state();
        let current_song = audio_player.current_song().cloned();
        let position = audio_player.position();

        div()
            .flex()
            .flex_col()
            .bg(rgb(0x1a1a1a))
            .size_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0x333333))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x888888))
                            .child(format!("{} songs", song_count)),
                    )
                    .child(
                        div()
                            .id("scan-button")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x333333))
                            .hover(|style| style.bg(rgb(0x444444)))
                            .cursor_pointer()
                            .text_sm()
                            .text_color(rgb(0xffffff))
                            .child("Scan for imports")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.scan_for_imports(cx);
                            })),
                    ),
            )
            .child(div().flex_1().child(self.list_view.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(rgb(0x333333))
                    .bg(rgb(0x222222))
                    .child(
                        div()
                            .id("play-pause-button")
                            .w_10()
                            .h_10()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(rgb(0x4ade80))
                            .hover(|style| style.bg(rgb(0x5aee90)))
                            .cursor_pointer()
                            .text_color(rgb(0x000000))
                            .text_lg()
                            .child(match playback_state {
                                PlaybackState::Playing => "⏸",
                                PlaybackState::Paused | PlaybackState::Stopped => "▶",
                            })
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.toggle_playback(cx);
                            })),
                    )
                    .child(div().flex_1().flex().flex_col().gap_1().map(|this| {
                        if let Some(song) = &current_song {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0xffffff))
                                    .child(song.title.clone()),
                            )
                            .child(
                                div().text_xs().text_color(rgb(0x888888)).child(
                                    song.artist
                                        .clone()
                                        .unwrap_or_else(|| "Unknown Artist".to_string()),
                                ),
                            )
                        } else {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x666666))
                                    .child("No track playing"),
                            )
                        }
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x888888))
                            .child(format_duration(position)),
                    ),
            )
    }
}

fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{}:{:02}", minutes, seconds)
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| cx.new(Player::new))
            .unwrap();
    });
}
