use std::path::PathBuf;
use std::time::Duration;

use gpui::*;
use player_core::{import_directory, load_library, save_library, Library, Song, SongId};
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
                let library = cx.new(|_cx| load_or_import_library());

                let list_view = cx.new(|cx| ListView::new(library, cx));

                Player { list_view }
            })
        })
        .unwrap();
    });
}

fn load_or_import_library() -> Library {
    // Try to load existing library from storage
    match load_library() {
        Ok(library) if !library.songs.is_empty() => {
            println!("Loaded {} tracks from library", library.songs.len());
            return library;
        }
        Ok(_) => {
            println!("Library is empty, will import...");
        }
        Err(e) => {
            eprintln!("Failed to load library: {:?}, will import...", e);
        }
    }

    // Fall back to importing from _import directory
    let library = import_from_directory();

    // Save the imported library
    if !library.songs.is_empty() {
        match save_library(&library) {
            Ok(()) => println!("Saved library to disk"),
            Err(e) => eprintln!("Failed to save library: {:?}", e),
        }
    }

    library
}

fn import_from_directory() -> Library {
    let mut library = Library::default();

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
