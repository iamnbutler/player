use std::time::Duration;

use gpui::*;
use player_core::{AudioFile, AudioFormat, Library, Song, SongId};
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
                // Create the library entity with sample data
                let library = cx.new(|_cx| create_sample_library());

                // Create the list view with the library
                let list_view = cx.new(|cx| ListView::new(library, cx));

                Player { list_view }
            })
        })
        .unwrap();
    });
}

fn create_sample_library() -> Library {
    let mut library = Library::default();

    let sample_songs = vec![
        (
            "Bohemian Rhapsody",
            Some("Queen"),
            Some("A Night at the Opera"),
            354,
        ),
        (
            "Stairway to Heaven",
            Some("Led Zeppelin"),
            Some("Led Zeppelin IV"),
            482,
        ),
        (
            "Hotel California",
            Some("Eagles"),
            Some("Hotel California"),
            391,
        ),
        ("Imagine", Some("John Lennon"), Some("Imagine"), 187),
        (
            "Smells Like Teen Spirit",
            Some("Nirvana"),
            Some("Nevermind"),
            301,
        ),
        (
            "Billie Jean",
            Some("Michael Jackson"),
            Some("Thriller"),
            294,
        ),
        (
            "Sweet Child O' Mine",
            Some("Guns N' Roses"),
            Some("Appetite for Destruction"),
            356,
        ),
        (
            "Comfortably Numb",
            Some("Pink Floyd"),
            Some("The Wall"),
            382,
        ),
        (
            "Like a Rolling Stone",
            Some("Bob Dylan"),
            Some("Highway 61 Revisited"),
            369,
        ),
        ("Hey Jude", Some("The Beatles"), Some("Single"), 431),
        ("Purple Rain", Some("Prince"), Some("Purple Rain"), 521),
        (
            "Wonderwall",
            Some("Oasis"),
            Some("(What's the Story) Morning Glory?"),
            259,
        ),
        (
            "Lose Yourself",
            Some("Eminem"),
            Some("8 Mile Soundtrack"),
            326,
        ),
        (
            "Take On Me",
            Some("a-ha"),
            Some("Hunting High and Low"),
            225,
        ),
        ("Don't Stop Believin'", Some("Journey"), Some("Escape"), 251),
    ];

    for (id, (title, artist, album, duration_secs)) in sample_songs.into_iter().enumerate() {
        let song = Song {
            id: SongId(id as u64),
            file: AudioFile {
                path: format!("/music/{}.mp3", title.to_lowercase().replace(' ', "_")).into(),
                format: AudioFormat::Mp3,
            },
            title: title.to_string(),
            artist: artist.map(String::from),
            album: album.map(String::from),
            track_number: Some((id + 1) as u32),
            duration: Duration::from_secs(duration_secs),
        };
        library.songs.insert(song.id, song);
    }

    library
}
