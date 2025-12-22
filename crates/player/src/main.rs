use futures::FutureExt;
use gpui::prelude::*;
use gpui::*;
use gpuikit::elements::icon_button::icon_button;
use gpuikit::layout::{h_stack, v_stack};
use gpuikit::DefaultIcons;
use gpuikit_theme::{ActiveTheme, Themeable};
use player_core::{
    ensure_directories, import_all_pending, problem_path, repair_problem_files_with_progress,
    save_library, AudioPlayer, AudioPlayerEvent, Library, LibraryReader, LoadedEntry,
    PlaybackState, RepairProgress, Song,
};
use std::time::Duration;
use ui::{ListView, ListViewEvent};

struct Player {
    library: Entity<Library>,
    list_view: Entity<ListView>,
    audio_player: Entity<AudioPlayer>,
    status_message: Option<String>,
    is_syncing: bool,
    sync_task: Option<Task<()>>,
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
            status_message: None,
            is_syncing: false,
            sync_task: None,
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

    fn set_status(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.status_message = Some(message.into());
        cx.notify();
    }

    fn clear_status(&mut self, cx: &mut Context<Self>) {
        self.status_message = None;
        cx.notify();
    }

    fn sync_library(&mut self, cx: &mut Context<Self>) {
        if self.is_syncing {
            return;
        }

        self.is_syncing = true;
        self.set_status("Starting sync...", cx);

        let library = self.library.clone();
        let (progress_tx, progress_rx) = smol::channel::unbounded::<RepairProgress>();

        let task = cx.spawn(async move |this, cx| {
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

            let problem_dir = problem_path();
            let has_problem_files = problem_dir.exists()
                && std::fs::read_dir(&problem_dir)
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(false);

            if has_problem_files {
                let progress_tx_clone = progress_tx.clone();
                let repair_task = cx.background_executor().spawn(async move {
                    repair_problem_files_with_progress(|progress| {
                        let _ = progress_tx_clone.send_blocking(progress);
                    })
                });

                let mut repair_task = repair_task.fuse();

                loop {
                    futures::select_biased! {
                        progress = progress_rx.recv().fuse() => {
                            if let Ok(progress) = progress {
                                let filename = progress.current_file
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                let _ = this.update(cx, |this, cx| {
                                    this.set_status(
                                        format!("Repairing {} ({}/{})", filename, progress.current, progress.total),
                                        cx,
                                    );
                                });
                            }
                        }
                        result = &mut repair_task => {
                            let (repair_successes, repair_failures) = result;
                            if !repair_successes.is_empty() {
                                let _ = this.update(cx, |this, cx| {
                                    this.set_status(format!("Repaired {} files", repair_successes.len()), cx);
                                });
                            }
                            if !repair_failures.is_empty() {
                                eprintln!("{} files could not be repaired:", repair_failures.len());
                                for failure in &repair_failures {
                                    eprintln!("  - {:?}: {}", failure.path, failure.reason);
                                }
                            }
                            break;
                        }
                    }
                }
            }

            let _ = this.update(cx, |this, cx| {
                this.set_status("Importing files...", cx);
            });

            let (results, lib) = cx
                .background_executor()
                .spawn(async move {
                    let results = import_all_pending(&mut lib);
                    (results, lib)
                })
                .await;
            let success_count = results.iter().filter(|r| r.is_ok()).count();
            let error_count = results.iter().filter(|r| r.is_err()).count();

            if success_count > 0 {
                let _ = this.update(cx, |this, cx| {
                    this.set_status(format!("Imported {} files", success_count), cx);
                });

                if let Err(e) = save_library(&lib) {
                    eprintln!("Failed to save library: {}", e);
                }

                let new_songs = lib.songs.clone();
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
                let _ = this.update(cx, |this, cx| {
                    this.set_status(format!("{} files moved to Problem folder", error_count), cx);
                });
            }

            let final_message = if success_count > 0 || error_count > 0 {
                "Sync complete".to_string()
            } else {
                "No new files".to_string()
            };

            let _ = this.update(cx, |this, cx| {
                this.set_status(&final_message, cx);
            });

            cx.background_executor().timer(Duration::from_secs(3)).await;

            let _ = this.update(cx, |this, cx| {
                this.is_syncing = false;
                this.sync_task = None;
                this.clear_status(cx);
            });
        });

        self.sync_task = Some(task);
    }
}

fn progress_bar(
    position: Duration,
    duration: Duration,
    is_playing: bool,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let progress = if duration.as_secs_f32() > 0.0 {
        (position.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let bg_color = theme.surface_secondary();
    let fg_color = theme.accent();

    canvas(
        move |bounds, _, _| bounds,
        move |bounds, _, window, _cx| {
            if is_playing {
                window.request_animation_frame();
            }

            let corner_radius = px(2.0);
            let progress_width = bounds.size.width * progress;

            window.paint_quad(gpui::PaintQuad {
                bounds,
                corner_radii: gpui::Corners::all(corner_radius),
                background: bg_color.into(),
                border_widths: gpui::Edges::default(),
                border_color: gpui::transparent_black(),
                border_style: gpui::BorderStyle::default(),
            });

            if progress > 0.0 {
                let progress_bounds = Bounds {
                    origin: bounds.origin,
                    size: gpui::Size {
                        width: progress_width,
                        height: bounds.size.height,
                    },
                };
                window.paint_quad(gpui::PaintQuad {
                    bounds: progress_bounds,
                    corner_radii: gpui::Corners::all(corner_radius),
                    background: fg_color.into(),
                    border_widths: gpui::Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: gpui::BorderStyle::default(),
                });
            }
        },
    )
    .h(px(4.0))
    .w_full()
}

impl Render for Player {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let audio_player = self.audio_player.read(cx);
        let playback_state = audio_player.state();
        let is_playing = playback_state == PlaybackState::Playing;
        let current_song = audio_player.current_song().cloned();
        let position = audio_player.position();

        let duration = current_song
            .as_ref()
            .map(|s| s.duration)
            .unwrap_or(Duration::ZERO);

        let status_message = self.status_message.clone();

        v_stack()
            .bg(theme.bg())
            .size_full()
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.list_view.clone()),
            )
            .child(
                v_stack()
                    .gap(rems(0.5))
                    .px(rems(0.75))
                    .py(rems(0.5))
                    .bg(theme.surface())
                    .child(
                        h_stack()
                            .items_center()
                            .gap(rems(0.5))
                            .child(
                                icon_button(
                                    "play-pause",
                                    match playback_state {
                                        PlaybackState::Playing => DefaultIcons::pause(),
                                        PlaybackState::Paused | PlaybackState::Stopped => {
                                            DefaultIcons::play()
                                        }
                                    },
                                )
                                .on_click(cx.listener(
                                    |this, _event, _window, cx| {
                                        this.toggle_playback(cx);
                                    },
                                )),
                            )
                            .child(v_stack().flex_1().gap(rems(0.125)).map(|this| {
                                if let Some(song) = &current_song {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.fg())
                                            .child(song.title.clone()),
                                    )
                                    .child(
                                        div().text_xs().text_color(theme.fg_muted()).child(
                                            song.artist
                                                .clone()
                                                .unwrap_or_else(|| "Unknown Artist".to_string()),
                                        ),
                                    )
                                } else {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.fg_disabled())
                                            .child("No track playing"),
                                    )
                                }
                            })),
                    )
                    .child(
                        h_stack()
                            .items_center()
                            .gap(rems(0.5))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.fg_muted())
                                    .w(rems(2.5))
                                    .child(format_duration(position)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(progress_bar(position, duration, is_playing, cx)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .text_xs()
                                    .text_color(theme.fg_muted())
                                    .w(rems(2.5))
                                    .child(format_duration(duration)),
                            ),
                    ),
            )
            .child(
                h_stack()
                    .items_center()
                    .justify_between()
                    .h(px(22.0))
                    .px(rems(0.5))
                    .bg(theme.surface_secondary())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.fg_muted())
                            .child(status_message.unwrap_or_default()),
                    )
                    .child(
                        div()
                            .id("sync-button")
                            .text_xs()
                            .text_color(if self.is_syncing {
                                theme.fg_disabled()
                            } else {
                                theme.fg_muted()
                            })
                            .when(!self.is_syncing, |el| {
                                el.cursor_pointer()
                                    .hover(|s| s.text_color(theme.fg()))
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.sync_library(cx);
                                    }))
                            })
                            .child(if self.is_syncing {
                                "Syncing..."
                            } else {
                                "Sync"
                            }),
                    ),
            )
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{}:{:02}", minutes, seconds)
}

fn main() {
    Application::new()
        .with_assets(gpuikit::assets())
        .run(|cx: &mut App| {
            gpuikit::init(cx);
            cx.open_window(WindowOptions::default(), |_window, cx| cx.new(Player::new))
                .unwrap();

            cx.activate(true);
        });
}
