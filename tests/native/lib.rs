use std::ffi::{c_char, c_int, CStr};

extern "C" {
    fn native_abandoned_batch() -> bool;
    fn native_nested_open(path: *const c_char, damaged: *const c_char) -> c_int;
    fn native_stream_callback_bytes(path: *const c_char, operation: c_int) -> c_int;
    fn native_worker_failure(path: *const c_char, kind: c_int) -> c_int;
}

pub fn nested_open(path: &CStr, damaged: Option<&CStr>) -> i32 {
    unsafe {
        native_nested_open(
            path.as_ptr(),
            damaged.map_or(std::ptr::null(), CStr::as_ptr),
        )
    }
}

pub fn worker_failure(path: &CStr, kind: i32) -> i32 {
    unsafe { native_worker_failure(path.as_ptr(), kind) }
}

pub fn stream_callback_bytes(path: &CStr, operation: i32) -> i32 {
    unsafe { native_stream_callback_bytes(path.as_ptr(), operation) }
}

pub fn abandoned_batch_discards_queued_work() -> bool {
    unsafe { native_abandoned_batch() }
}
