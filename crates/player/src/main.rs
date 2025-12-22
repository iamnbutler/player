use std::path::PathBuf;
use std::time::Duration;

use gpui::*;
use player_core::{import_directory, Library, Song, SongId};
use ui::ListView;

struct Player {
    list_view: Entity<ListView>,
}

impl Render for Player {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(rgb(0x1a1a1a))
            .size_full()
            .child(self.list_view.clone())
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|cx| {
                // Import tracks from the _import directory
                let library = cx.new(|_cx| load_library_from_import());

                // Create the list view with the library
                let list_view = cx.new(|cx| ListView::new(library, cx));

                Player { list_view }
            })
        })
        .unwrap();
    });
}

fn load_library_from_import() -> Library {
    let mut library = Library::default();

    // Path to the import directory
    let import_path = PathBuf::from("_import");

    match import_directory(&import_path) {
        Ok(imported_files) => {
            for (id, imported) in imported_files.into_iter().enumerate() {
                let song = Song {
                    id: SongId(id as u64),
                    file: imported.file,
                    title: imported
                        .metadata
                        .title
                        .unwrap_or_else(|| "Unknown Title".to_string()),
                    artist: imported.metadata.artist.or(imported.metadata.album_artist),
                    album: imported.metadata.album,
                    track_number: imported.metadata.track_number,
                    duration: imported.metadata.duration.unwrap_or(Duration::ZERO),
                };
                library.songs.insert(song.id, song);
            }
            println!("Imported {} tracks", library.songs.len());
        }
        Err(e) => {
            eprintln!("Failed to import directory: {:?}", e);
        }
    }

    library
}
