use std::ffi::CString;
use unrar_sys as _; // The native test harness calls the production DLL.

fn archive_path() -> CString {
    CString::new("data/version.rar").unwrap()
}

#[test]
fn callback_can_open_an_independent_archive_on_the_same_thread() {
    assert_eq!(unrar_native_tests::nested_open(&archive_path(), None), 0);
}

#[test]
fn overlapping_archive_operations_preserve_their_own_errors() {
    let mut damaged = std::fs::read("data/primary-stream.cbr").unwrap();
    let marker = b"page bytes";
    let start = damaged
        .windows(marker.len())
        .position(|bytes| bytes == marker)
        .unwrap();
    damaged[start] ^= 1;
    let dir = tempfile::tempdir_in(".").unwrap();
    let bad = dir.path().join("bad.cbr");
    std::fs::write(&bad, damaged).unwrap();
    let bad = CString::new(bad.to_str().unwrap()).unwrap();
    assert_eq!(
        unrar_native_tests::nested_open(&archive_path(), Some(&bad)),
        0
    );
    let fresh = unrar::Archive::new("data/version.rar")
        .open_for_processing()
        .unwrap()
        .read_header()
        .unwrap()
        .unwrap()
        .read_bounded(11)
        .unwrap();
    assert_eq!(fresh, b"unrar-0.4.0");
}

#[test]
fn native_worker_errors_settle_the_batch_and_reach_the_owner() {
    let path = archive_path();
    for kind in 0..4 {
        assert_eq!(
            unrar_native_tests::worker_failure(&path, kind),
            0,
            "failure {kind}"
        );
    }
}

#[test]
fn close_during_unwind_discards_unstarted_borrowed_tasks() {
    assert!(unrar_native_tests::abandoned_batch_discards_queued_work());
}
