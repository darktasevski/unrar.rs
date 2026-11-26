const ARCHIVE: &str = "data/primary-stream.cbr";

#[test]
fn bounded_read_returns_only_primary_file_bytes() {
    let entry = unrar::Archive::new(ARCHIVE)
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    assert_eq!(entry.read_bounded(10), Ok(b"page bytes".to_vec()));
}

#[test]
fn empty_primary_with_a_stream_fits_a_zero_cap() {
    let first = unrar::Archive::new(ARCHIVE)
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    let empty = first.skip().unwrap().read_header().unwrap().unwrap();
    assert_eq!(empty.read_bounded(0), Ok(Vec::new()));
}

#[test]
fn prefix_read_drains_primary_and_advances_to_empty_entry() {
    let first = unrar::Archive::new(ARCHIVE)
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    let mut buffer = [0; 4];
    let (count, next) = first.read_prefix(&mut buffer).unwrap();
    assert_eq!((buffer, count), (*b"page", 10));
    let empty = next.read_header().unwrap().unwrap();
    assert_eq!(empty.entry().filename.to_str(), Some("empty"));
    let (count, end) = empty.read_prefix(&mut []).unwrap();
    assert_eq!(count, 0);
    assert!(end.read_header().unwrap().is_none());
}
