use std::ops::Range;

use gpui::{
    div, prelude::*, px, rems, uniform_list, App, Context, Entity, EventEmitter, IntoElement,
    Render, SharedString, UniformListScrollHandle, Window,
};
use gpuikit::layout::{h_stack, v_stack};
use gpuikit_theme::{ActiveTheme, Themeable};
use player_core::{Library, Song, SongId, SortOrder};

pub struct ListView {
    library: Entity<Library>,
    scroll_handle: UniformListScrollHandle,
    sort_order: SortOrder,
    playing_song_id: Option<SongId>,
    selected_index: Option<usize>,
}

pub enum ListViewEvent {
    SongSelected(Song),
    SongDoubleClicked(Song),
}

impl EventEmitter<ListViewEvent> for ListView {}

impl ListView {
    pub fn new(library: Entity<Library>, _cx: &mut Context<Self>) -> Self {
        Self {
            library,
            scroll_handle: UniformListScrollHandle::new(),
            sort_order: SortOrder::default(),
            playing_song_id: None,
            selected_index: None,
        }
    }

    pub fn sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = sort_order;
        self
    }

    pub fn set_playing_song(&mut self, song_id: Option<SongId>, cx: &mut Context<Self>) {
        self.playing_song_id = song_id;
        cx.notify();
    }

    pub fn next_song(&self, cx: &App) -> Option<Song> {
        let playing_id = self.playing_song_id?;
        let library = self.library.read(cx);
        let songs: Vec<Song> = library.list(self.sort_order);

        let current_index = songs.iter().position(|s| s.id == playing_id)?;
        songs.get(current_index + 1).cloned()
    }
}

impl Render for ListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let library = self.library.read(cx);

        let songs: Vec<Song> = library.list(self.sort_order);
        let song_count = songs.len();
        let playing_song_id = self.playing_song_id;
        let selected_index = self.selected_index;

        let header_text_color = theme.fg_muted();

        v_stack()
            .size_full()
            .child(
                h_stack()
                    .h(px(20.0))
                    .items_center()
                    .w_full()
                    .px(rems(0.5))
                    .bg(theme.surface())
                    .border_b_1()
                    .border_color(theme.border())
                    .child(
                        div()
                            .w(rems(2.5))
                            .text_xs()
                            .text_color(header_text_color)
                            .overflow_hidden()
                            .child("#"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(header_text_color)
                            .overflow_hidden()
                            .child("Title"),
                    )
                    .child(
                        div()
                            .w(rems(3.0))
                            .text_xs()
                            .text_color(header_text_color)
                            .overflow_hidden()
                            .child("Time"),
                    )
                    .child(
                        div()
                            .w(rems(10.0))
                            .text_xs()
                            .text_color(header_text_color)
                            .overflow_hidden()
                            .child("Artist"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(header_text_color)
                            .overflow_hidden()
                            .child("Album"),
                    ),
            )
            .child(
                div().flex_1().child(
                    uniform_list(
                        "track-list",
                        song_count,
                        cx.processor(move |_this, range: Range<usize>, _window, cx| {
                            let theme = cx.theme();
                            let mut items = Vec::new();

                            for ix in range {
                                if let Some(song) = songs.get(ix) {
                                    let title: SharedString = song.title.clone().into();
                                    let artist: SharedString =
                                        song.artist.clone().unwrap_or_default().into();
                                    let album: SharedString =
                                        song.album.clone().unwrap_or_default().into();
                                    let track_number: SharedString = song
                                        .track_number
                                        .map(|n| n.to_string())
                                        .unwrap_or_default()
                                        .into();

                                    let is_playing = playing_song_id == Some(song.id);
                                    let is_selected = selected_index == Some(ix);

                                    let bg_color = if is_playing {
                                        theme.accent_bg()
                                    } else if is_selected {
                                        theme.selection()
                                    } else if ix % 2 == 0 {
                                        theme.bg()
                                    } else {
                                        theme.surface()
                                    };

                                    let title_color = if is_playing {
                                        theme.accent()
                                    } else {
                                        theme.fg()
                                    };

                                    let song_for_click = song.clone();

                                    items.push(
                                        h_stack()
                                            .id(ix)
                                            .h(px(20.0))
                                            .items_center()
                                            .w_full()
                                            .px(rems(0.5))
                                            .bg(bg_color)
                                            .child(
                                                div()
                                                    .w(rems(2.5))
                                                    .text_xs()
                                                    .text_color(theme.fg_muted())
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .when(is_playing, |el| {
                                                        el.text_color(theme.accent()).child("▶")
                                                    })
                                                    .when(!is_playing, |el| el.child(track_number)),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_xs()
                                                    .text_color(title_color)
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .child(title),
                                            )
                                            .child(
                                                div()
                                                    .w(rems(3.0))
                                                    .text_xs()
                                                    .text_color(theme.fg_disabled())
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .child(format_duration(song.duration)),
                                            )
                                            .child(
                                                div()
                                                    .w(rems(10.0))
                                                    .text_xs()
                                                    .text_color(theme.fg_muted())
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .child(artist),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_xs()
                                                    .text_color(theme.fg_muted())
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .child(album),
                                            )
                                            .on_click(cx.listener(
                                                move |this, event: &gpui::ClickEvent, _window, cx| {
                                                    this.selected_index = Some(ix);

                                                    if event.click_count() >= 2 {
                                                        cx.emit(ListViewEvent::SongDoubleClicked(
                                                            song_for_click.clone(),
                                                        ));
                                                    } else {
                                                        cx.emit(ListViewEvent::SongSelected(
                                                            song_for_click.clone(),
                                                        ));
                                                    }
                                                    cx.notify();
                                                },
                                            )),
                                    );
                                }
                            }

                            items
                        }),
                    )
                    .track_scroll(self.scroll_handle.clone())
                    .size_full(),
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
