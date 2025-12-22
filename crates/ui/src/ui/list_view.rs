use gpui::{
    div, prelude::*, uniform_list, Context, Entity, IntoElement, Render, SharedString,
    UniformListScrollHandle, Window,
};
use player_core::Library;

pub struct ListView {
    library: Entity<Library>,
    scroll_handle: UniformListScrollHandle,
}

impl ListView {
    pub fn new(library: Entity<Library>, _cx: &mut Context<Self>) -> Self {
        Self {
            library,
            scroll_handle: UniformListScrollHandle::new(),
        }
    }
}

impl Render for ListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let library = self.library.read(cx);

        // Collect songs into a vector for rendering
        let songs: Vec<_> = library.songs.values().cloned().collect();
        let song_count = songs.len();

        div().size_full().child(
            uniform_list("track-list", song_count, {
                move |range, _window, _cx| {
                    let mut items = Vec::new();

                    for ix in range {
                        if let Some(song) = songs.get(ix) {
                            let title: SharedString = song.title.clone().into();
                            let artist: SharedString = song
                                .artist
                                .clone()
                                .unwrap_or_else(|| "Unknown Artist".to_string())
                                .into();

                            items.push(
                                div()
                                    .id(ix)
                                    .h_6()
                                    .items_center()
                                    .flex()
                                    .w_full()
                                    .px_2()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(gpui::rgb(0x333333)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_sm()
                                            .text_color(gpui::rgb(0xffffff))
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_xs()
                                            .text_color(gpui::rgb(0x888888))
                                            .child(artist),
                                    ),
                            );
                        }
                    }

                    items
                }
            })
            .track_scroll(&self.scroll_handle)
            .size_full(),
        )
    }
}
