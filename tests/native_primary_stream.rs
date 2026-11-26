use std::ffi::CString;

const ARCHIVE: &str = "data/primary-stream.cbr";

#[test]
fn auxiliary_decoder_reports_bytes_only_for_the_legacy_operation() {
    let path = CString::new(ARCHIVE).unwrap();
    assert_eq!(
        unrar_native_tests::stream_callback_bytes(&path, unrar_sys::RAR_TEST),
        12
    );
    assert_eq!(
        unrar_native_tests::stream_callback_bytes(&path, unrar_sys::RAR_TEST_PRIMARY),
        0
    );
}

#[test]
fn auxiliary_decoder_still_verifies_the_stream_checksum() {
    let mut archive = std::fs::read(ARCHIVE).unwrap();
    let needle = b"stream bytes";
    let start = archive
        .windows(needle.len())
        .position(|part| part == needle)
        .unwrap();
    archive[start] ^= 1;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stream.cbr");
    std::fs::write(&path, archive).unwrap();
    let path = CString::new(path.to_str().unwrap()).unwrap();
    assert_eq!(
        unrar_native_tests::stream_callback_bytes(&path, unrar_sys::RAR_TEST_PRIMARY),
        -2
    );
}
