use unrar::error::{Code, UnrarError, When};
use unrar::{Archive, BoundedReadError};

fn read(archive: Archive, cap: usize) -> Result<Vec<u8>, BoundedReadError> {
    archive
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .read_bounded(cap)
}

#[test]
fn bounded_entry() {
    assert_eq!(
        read(Archive::new("data/version.rar"), 11).unwrap(),
        b"unrar-0.4.0"
    );
    for cap in [10, 0] {
        assert_eq!(
            read(Archive::new("data/version.rar"), cap),
            Err(BoundedReadError::LimitExceeded)
        );
    }
}

#[test]
fn empty_entry_at_zero_cap() {
    assert_eq!(
        read(Archive::new("data/bounded-empty.rar"), 0).unwrap(),
        b""
    );
}

#[test]
fn password_status() {
    assert_eq!(
        read(Archive::with_password("data/crypted.rar", "unrar"), 128).unwrap(),
        b"target\nCargo.lock\n"
    );
    for (archive, code) in [
        (Archive::new("data/crypted.rar"), Code::MissingPassword),
        (
            Archive::with_password("data/crypted.rar", "incorrect"),
            Code::BadData,
        ),
    ] {
        assert_eq!(
            read(archive, 128),
            Err(BoundedReadError::Archive(UnrarError::from(
                code,
                When::Process
            )))
        );
    }
}

#[test]
fn split_entry() {
    let name = "bounded-split.part1.rar";
    let path = std::path::Path::new("data").join(name);
    let bytes: Vec<_> = (0..=255).cycle().take(1536).collect();
    assert_eq!(read(Archive::new(&path), bytes.len()).unwrap(), bytes);
    let temp = tempfile::tempdir().unwrap();
    let only_first = temp.path().join(name);
    std::fs::copy(&path, &only_first).unwrap();
    assert_eq!(
        read(Archive::new(&only_first), 1536),
        Err(BoundedReadError::Archive(UnrarError::from(
            Code::EOpen,
            When::Process
        )))
    );
}

#[test]
fn corrupted_entry_discards_delivered_bytes() {
    use std::io::Read;
    let temp = tempfile::tempdir().unwrap();
    let first = "bounded-split.part1.rar";
    let second = "bounded-split.part2.rar";
    let path = temp.path().join(first);
    std::fs::copy(std::path::Path::new("data").join(first), &path).unwrap();
    let mut damaged = std::fs::read(std::path::Path::new("data").join(second)).unwrap();
    let payload = damaged.windows(4).position(|w| w == [0, 1, 2, 3]).unwrap();
    damaged[payload] ^= 1;
    std::fs::write(temp.path().join(second), damaged).unwrap();
    let mut stream = Archive::new(&path)
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .read_streaming();
    let mut prefix = [0; 8];
    stream.read_exact(&mut prefix).unwrap();
    assert_eq!(prefix, [0, 1, 2, 3, 4, 5, 6, 7]);
    drop(stream);
    let entry = Archive::new(&path)
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap();
    let result = entry.read_prefix(&mut [0; 8]);
    assert_eq!(
        result.err(),
        Some(UnrarError::from(Code::BadData, When::Process))
    );
    assert_eq!(
        read(Archive::new(&path), 1536),
        Err(BoundedReadError::Archive(UnrarError::from(
            Code::BadData,
            When::Process
        )))
    );
}

// Fixtures use RARLAB 7.23. The empty archive stores a regular zero-byte file:
// rar a -cfg- -idq -ma5 -m0 bounded-empty.rar empty
// The split fixture stores bytes 0..=255 repeated six times in a file called bytes:
// rar a -cfg- -idq -ma5 -m0 -v1k bounded-split.rar bytes
