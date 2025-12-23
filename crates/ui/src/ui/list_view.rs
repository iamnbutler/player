use std::ops::Range;

use gpui::{
    actions, div, prelude::*, px, rems, uniform_list, App, Context, Entity, EventEmitter,
    FocusHandle, Focusable, IntoElement, KeyBinding, Render, ScrollStrategy, SharedString,
    UniformListScrollHandle, Window,
};
use gpuikit::layout::{h_stack, v_stack};
use gpuikit_theme::{ActiveTheme, Themeable};
use player_core::{Library, Song, SongId, SortOrder};

actions!(
    list_view,
    [
        SelectNext,
        SelectPrevious,
        SelectFirst,
        SelectLast,
        PageDown,
        PageUp,
        PlaySelected,
        TogglePlayback,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, Some("ListView")),
        KeyBinding::new("up", SelectPrevious, Some("ListView")),
        KeyBinding::new("j", SelectNext, Some("ListView")),
        KeyBinding::new("k", SelectPrevious, Some("ListView")),
        KeyBinding::new("home", SelectFirst, Some("ListView")),
        KeyBinding::new("end", SelectLast, Some("ListView")),
        KeyBinding::new("cmd-up", SelectFirst, Some("ListView")),
        KeyBinding::new("cmd-down", SelectLast, Some("ListView")),
        KeyBinding::new("pagedown", PageDown, Some("ListView")),
        KeyBinding::new("pageup", PageUp, Some("ListView")),
        KeyBinding::new("enter", PlaySelected, Some("ListView")),
        KeyBinding::new("space", TogglePlayback, Some("ListView")),
    ]);
}

pub struct ListView {
    library: Entity<Library>,
    scroll_handle: UniformListScrollHandle,
    sort_order: SortOrder,
    playing_song_id: Option<SongId>,
    selected_index: Option<usize>,
    focus_handle: FocusHandle,
}

pub enum ListViewEvent {
    SongSelected(Song),
    SongDoubleClicked(Song),
    PlaySelected(Song),
    TogglePlayback,
}

impl EventEmitter<ListViewEvent> for ListView {}

impl Focusable for ListView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ListView {
    pub fn new(library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        Self {
            library,
            scroll_handle: UniformListScrollHandle::new(),
            sort_order: SortOrder::default(),
            playing_song_id: None,
            selected_index: None,
            focus_handle: cx.focus_handle(),
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

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window);
        cx.notify();
    }

    fn song_count(&self, cx: &App) -> usize {
        self.library.read(cx).list(self.sort_order).len()
    }

    fn get_song_at_index(&self, index: usize, cx: &App) -> Option<Song> {
        let library = self.library.read(cx);
        let songs: Vec<Song> = library.list(self.sort_order);
        songs.get(index).cloned()
    }

    fn selected_song(&self, cx: &App) -> Option<Song> {
        let index = self.selected_index?;
        self.get_song_at_index(index, cx)
    }

    fn select_next(&mut self, _: &SelectNext, _window: &mut Window, cx: &mut Context<Self>) {
        let count = self.song_count(cx);
        if count == 0 {
            return;
        }

        let new_index = match self.selected_index {
            Some(index) => (index + 1).min(count.saturating_sub(1)),
            None => 0,
        };

        self.select_index(new_index, cx);
    }

    fn select_previous(
        &mut self,
        _: &SelectPrevious,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.song_count(cx);
        if count == 0 {
            return;
        }

        let new_index = match self.selected_index {
            Some(index) => index.saturating_sub(1),
            None => 0,
        };

        self.select_index(new_index, cx);
    }

    fn select_first(&mut self, _: &SelectFirst, _window: &mut Window, cx: &mut Context<Self>) {
        let count = self.song_count(cx);
        if count == 0 {
            return;
        }

        self.select_index(0, cx);
    }

    fn select_last(&mut self, _: &SelectLast, _window: &mut Window, cx: &mut Context<Self>) {
        let count = self.song_count(cx);
        if count == 0 {
            return;
        }

        self.select_index(count.saturating_sub(1), cx);
    }

    fn page_down(&mut self, _: &PageDown, _window: &mut Window, cx: &mut Context<Self>) {
        let count = self.song_count(cx);
        if count == 0 {
            return;
        }

        let page_size = 20;
        let new_index = match self.selected_index {
            Some(index) => (index + page_size).min(count.saturating_sub(1)),
            None => page_size.min(count.saturating_sub(1)),
        };

        self.select_index(new_index, cx);
    }

    fn page_up(&mut self, _: &PageUp, _window: &mut Window, cx: &mut Context<Self>) {
        let count = self.song_count(cx);
        if count == 0 {
            return;
        }

        let page_size = 20;
        let new_index = match self.selected_index {
            Some(index) => index.saturating_sub(page_size),
            None => 0,
        };

        self.select_index(new_index, cx);
    }

    fn play_selected(&mut self, _: &PlaySelected, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(song) = self.selected_song(cx) {
            cx.emit(ListViewEvent::PlaySelected(song));
        }
    }

    fn toggle_playback(
        &mut self,
        _: &TogglePlayback,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.emit(ListViewEvent::TogglePlayback);
    }

    fn select_index(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_index = Some(index);
        self.scroll_handle
            .scroll_to_item(index, ScrollStrategy::Center);

        if let Some(song) = self.get_song_at_index(index, cx) {
            cx.emit(ListViewEvent::SongSelected(song));
        }

        cx.notify();
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
            .key_context("ListView")
            .id("list-view")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::select_first))
            .on_action(cx.listener(Self::select_last))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::play_selected))
            .on_action(cx.listener(Self::toggle_playback))
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
                                                move |this, event: &gpui::ClickEvent, window, cx| {
                                                    this.selected_index = Some(ix);
                                                    this.focus_handle.focus(window);

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
