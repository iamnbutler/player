mod fixtures;

use fixtures::mp3_fixture;
use player_core::import::{import_file, ImportError};
use player_core::AudioFormat;

#[test]
fn import_mp3_reads_metadata() {
    let path = mp3_fixture();
    let imported = import_file(&path).unwrap();

    assert_eq!(imported.file.format, AudioFormat::Mp3);
    assert_eq!(imported.file.path, path);
}

#[test]
fn import_unknown_format_returns_error() {
    let path = fixtures::fixture_path("unknown_format.txt");
    let result = import_file(&path);

    assert!(matches!(result, Err(ImportError::UnknownFormat)));
}

#[test]
fn import_nonexistent_file_returns_error() {
    let path = fixtures::fixture_path("does_not_exist.mp3");
    let result = import_file(&path);

    assert!(matches!(result, Err(ImportError::Id3Error(_))));
}
