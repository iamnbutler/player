use std::path::PathBuf;

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub fn mp3_fixture() -> PathBuf {
    fixture_path("mp3_700KB.mp3")
}

// TODO: Future fixtures needed:
// - MP3 with full ID3v2.4 tags (title, artist, album, track number, year, genre)
// - MP3 with ID3v1 tags only
// - MP3 with no tags at all
// - MP3 with unicode metadata (Japanese, Russian, Chinese)
// - MP3 with embedded album art
// - Corrupted MP3 (invalid frame headers)
// - M4B audiobook with chapters
// - M4B audiobook without chapters
