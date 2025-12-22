use gpui::*;
use player_core::{
    ensure_directories, import_all_pending, import_path, save_library, Library, LibraryReader,
    LoadedEntry,
};
use ui::ListView;

struct Player {
    library: Entity<Library>,
    list_view: Entity<ListView>,
}

impl Player {
    fn new(cx: &mut Context<Self>) -> Self {
        // Ensure all directories exist
        if let Err(e) = ensure_directories() {
            eprintln!("Failed to create directories: {}", e);
        }

        // Create the library entity
        let library = cx.new(|_cx| Library::new());

        // Stream load the library from disk
        Self::stream_load_library(library.clone(), cx);

        // Create the list view
        let list_view = cx.new(|cx| ListView::new(library.clone(), cx));

        Player { library, list_view }
    }

    /// Stream load the library in chunks to avoid blocking
    fn stream_load_library(library: Entity<Library>, cx: &mut Context<Self>) {
        cx.spawn(async move |_this, cx| {
            // Open the library reader
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

                        // Process in batches to avoid holding the lock too long
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
                    LoadedEntry::Meta(_) => {
                        // Metadata is informational
                    }
                    LoadedEntry::Skipped { line_number, error } => {
                        eprintln!("Warning: Skipped line {}: {}", line_number, error);
                    }
                }
            }

            // Add any remaining songs
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

    /// Scan for new files in the Import directory
    fn scan_for_imports(&mut self, cx: &mut Context<Self>) {
        let library = self.library.clone();

        cx.spawn(async move |_this, cx| {
            println!("Scanning {} for new files...", import_path().display());

            // Load current library state into a temporary library for import
            let mut lib = Library::new();

            // Copy current library state
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

            // Run import
            let results = import_all_pending(&mut lib);

            // Report results
            let success_count = results.iter().filter(|r| r.is_ok()).count();
            let error_count = results.iter().filter(|r| r.is_err()).count();

            if success_count > 0 {
                println!("Imported {} new files", success_count);

                // Save updated library
                if let Err(e) = save_library(&lib) {
                    eprintln!("Failed to save library: {}", e);
                }

                // Update the library entity with new songs
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

        div()
            .flex()
            .flex_col()
            .bg(rgb(0x1a1a1a))
            .size_full()
            // Header
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
            // List view
            .child(div().flex_1().child(self.list_view.clone()))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|cx| Player::new(cx))
        })
        .unwrap();
    });
}
