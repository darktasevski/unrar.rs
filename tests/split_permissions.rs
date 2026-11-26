#[cfg(unix)]
#[test]
fn split_windows_file_keeps_read_only_unix_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let ordinary = dir.path().join("ordinary");
    std::fs::write(&ordinary, b"").unwrap();
    let read_bits = ordinary.metadata().unwrap().permissions().mode() & 0o444;

    let extracted = dir.path().join("extracted");
    unrar::Archive::new("data/windows-mode.part1.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .extract_to(&extracted)
        .unwrap();
    assert_eq!(
        extracted.metadata().unwrap().permissions().mode() & 0o777,
        read_bits
    );
}
