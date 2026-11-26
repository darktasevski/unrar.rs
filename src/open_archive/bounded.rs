use super::*;

impl OpenArchive<Process, CursorBeforeFile> {
    /// Runs the native entry read and output callback on the calling thread, then closes the archive.
    /// The native decoder and hash implementation can still use worker threads.
    /// The limit bounds retained primary-entry bytes, including across volume changes.
    pub fn read_bounded(self, max_bytes: usize) -> Result<Vec<u8>, BoundedReadError> {
        let mut output = BoundedOutput::new(max_bytes);
        let processed = self.process_primary(&mut output);
        output.finish(processed, self.handle.close())
    }

    /// Fills the beginning of `prefix` with primary file bytes and drains the entry
    /// on the calling thread. Returns the full primary byte count and the next cursor.
    /// When the entry is shorter than the buffer, its unused suffix is untouched.
    pub fn read_prefix(
        self,
        prefix: &mut [u8],
    ) -> UnrarResult<(u64, OpenArchive<Process, CursorBeforeHeader>)> {
        let mut output = PrefixOutput {
            remaining: prefix,
            delivered: 0,
            error: None,
        };
        let processed = self.process_primary(&mut output);
        let status = output.error.unwrap_or(processed);
        if status != Code::Success {
            return Err(UnrarError::from(status, When::Process));
        }
        Ok((output.delivered, self.next_header()))
    }

    fn process_primary<T: PrimaryOutput>(&self, output: &mut T) -> Code {
        Code::from_status(unsafe {
            self.handle.with_callback(callback::<T>, output, |raw| {
                pathed::process_file(raw, native::RAR_TEST_PRIMARY, None, None)
            })
        })
    }
}

trait PrimaryOutput {
    unsafe fn on_data(&mut self, ptr: *const u8, length: native::LPARAM) -> c_int;
}

fn span_length(ptr: *const u8, length: native::LPARAM) -> Result<usize, Code> {
    let len = usize::try_from(length).map_err(|_| Code::BadData)?;
    if len != 0 && ptr.is_null() {
        return Err(Code::BadData);
    }
    Ok(len)
}

struct PrefixOutput<'a> {
    remaining: &'a mut [u8],
    delivered: u64,
    error: Option<Code>,
}

impl PrimaryOutput for PrefixOutput<'_> {
    unsafe fn on_data(&mut self, ptr: *const u8, length: native::LPARAM) -> c_int {
        let Ok(len) = span_length(ptr, length) else {
            self.error = Some(Code::BadData);
            return -1;
        };
        let keep = self.remaining.len().min(len);
        if keep > 0 {
            let (dest, rest) = std::mem::take(&mut self.remaining).split_at_mut(keep);
            dest.copy_from_slice(unsafe { std::slice::from_raw_parts(ptr, keep) });
            self.remaining = rest;
        }
        self.delivered = self.delivered.saturating_add(len as u64);
        0
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    max_bytes: usize,
    error: Option<BoundedReadError>,
}

impl BoundedOutput {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
            error: None,
        }
    }

    fn finish(self, processed: Code, closed: Code) -> Result<Vec<u8>, BoundedReadError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        for result in [processed, closed] {
            if result != Code::Success {
                return Err(archive_error(result));
            }
        }
        Ok(self.bytes)
    }

    // UnRAR lends this span until its callback returns. Reserve against delivered
    // bytes before borrowing it; the on-disk header does not authorize an allocation.
    unsafe fn receive(
        &mut self,
        ptr: *const u8,
        length: native::LPARAM,
        reserve: impl FnOnce(&mut Vec<u8>, usize) -> Result<(), std::collections::TryReserveError>,
    ) -> Result<(), BoundedReadError> {
        let len = span_length(ptr, length).map_err(archive_error)?;
        if len == 0 {
            return Ok(());
        }
        if len > self.max_bytes - self.bytes.len() {
            return Err(BoundedReadError::LimitExceeded);
        }
        reserve(&mut self.bytes, len).map_err(|_| archive_error(Code::NoMemory))?;
        self.bytes
            .extend_from_slice(unsafe { std::slice::from_raw_parts(ptr, len) });
        Ok(())
    }
}

fn archive_error(code: Code) -> BoundedReadError {
    BoundedReadError::Archive(UnrarError::from(code, When::Process))
}

impl PrimaryOutput for BoundedOutput {
    unsafe fn on_data(&mut self, ptr: *const u8, length: native::LPARAM) -> c_int {
        unsafe { self.process(ptr, length, Vec::try_reserve) }
    }
}

extern "C" fn callback<T: PrimaryOutput>(
    msg: native::UINT,
    state: native::LPARAM,
    p1: native::LPARAM,
    p2: native::LPARAM,
) -> c_int {
    if state == 0 {
        return -1;
    }
    let output = unsafe { &mut *(state as *mut T) };
    match msg {
        native::UCM_CHANGEVOLUMEW => handle_volume_change(p2),
        native::UCM_PROCESSDATA => unsafe { output.on_data(p1 as *const u8, p2) },
        _ => 0,
    }
}

impl BoundedOutput {
    unsafe fn process(
        &mut self,
        ptr: *const u8,
        length: native::LPARAM,
        reserve: impl FnOnce(&mut Vec<u8>, usize) -> Result<(), std::collections::TryReserveError>,
    ) -> c_int {
        if self.error.is_some() {
            return -1;
        }
        match unsafe { self.receive(ptr, length, reserve) } {
            Ok(()) => 0,
            Err(error) => {
                self.error = Some(error);
                -1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send(
        output: &mut BoundedOutput,
        msg: native::UINT,
        ptr: *const u8,
        len: native::LPARAM,
    ) -> c_int {
        callback::<BoundedOutput>(
            msg,
            output as *mut _ as native::LPARAM,
            ptr as native::LPARAM,
            len,
        )
    }

    #[test]
    fn cumulative_cap_aborts_without_retaining_the_crossing_span() {
        let mut output = BoundedOutput::new(3);
        assert_eq!(
            send(&mut output, native::UCM_PROCESSDATA, b"ab".as_ptr(), 2),
            0
        );
        assert_eq!(
            send(&mut output, native::UCM_PROCESSDATA, b"cd".as_ptr(), 2),
            -1
        );
        assert_eq!(output.bytes, b"ab");
        assert_eq!(
            send(&mut output, native::UCM_PROCESSDATA, b"x".as_ptr(), 1),
            -1
        );
        assert_eq!(
            output.finish(Code::Unknown, Code::Success),
            Err(BoundedReadError::LimitExceeded)
        );
    }

    #[test]
    fn validates_native_spans_before_borrowing() {
        let null = std::ptr::null();
        let mut empty = BoundedOutput::new(0);
        assert_eq!(send(&mut empty, native::UCM_PROCESSDATA, null, 0), 0);
        for (ptr, len) in [(null, 1), (b"x".as_ptr(), -1)] {
            let mut output = BoundedOutput::new(0);
            assert_eq!(send(&mut output, native::UCM_PROCESSDATA, ptr, len), -1);
            assert_eq!(output.error, Some(archive_error(Code::BadData)));
        }
    }

    #[test]
    fn accepted_span_uses_reserved_storage() {
        let mut output = BoundedOutput::new(4096);
        let payload = [0x5A; 4096];
        let mut reserved = std::ptr::null();
        let reply = unsafe {
            output.process(
                payload.as_ptr(),
                payload.len() as native::LPARAM,
                |vec, len| {
                    vec.try_reserve(len)?;
                    reserved = vec.as_ptr();
                    Ok(())
                },
            )
        };
        assert_eq!(reply, 0);
        assert_eq!(output.bytes.as_ptr(), reserved);
        assert_eq!(output.bytes, payload);
    }

    #[test]
    fn rejected_allocation_aborts_without_appending() {
        let mut output = BoundedOutput::new(8);
        assert_eq!(
            send(&mut output, native::UCM_PROCESSDATA, b"one".as_ptr(), 3),
            0
        );
        let reply =
            unsafe { output.process(b"two".as_ptr(), 3, |vec, _| vec.try_reserve(usize::MAX)) };
        assert_eq!(reply, -1);
        assert_eq!(output.bytes, b"one");
        assert_eq!(
            output.finish(Code::Unknown, Code::Success),
            Err(archive_error(Code::NoMemory))
        );
    }

    #[test]
    fn dictionary_and_volume_messages_need_no_borrowed_path() {
        let mut output = BoundedOutput::new(0);
        let null = std::ptr::null();
        assert_eq!(send(&mut output, native::UCM_LARGEDICT, null, 0), 0);
        assert_eq!(
            send(
                &mut output,
                native::UCM_CHANGEVOLUMEW,
                null,
                native::RAR_VOL_NOTIFY
            ),
            0
        );
        assert_eq!(
            send(
                &mut output,
                native::UCM_CHANGEVOLUMEW,
                null,
                native::RAR_VOL_ASK
            ),
            -1
        );
    }

    #[test]
    fn statuses_cover_new_and_unrecognized_dll_values() {
        assert_eq!(Code::from_status(25), Code::LargeDict);
        assert_eq!(Code::from_status(99), Code::Unknown);
        assert_eq!(
            BoundedOutput::new(1).finish(Code::Success, Code::EClose),
            Err(archive_error(Code::EClose))
        );
    }

    #[test]
    fn callback_scope_unregisters_on_success_and_error() {
        extern "C" fn abort_on_data(
            msg: native::UINT,
            _: native::LPARAM,
            _: native::LPARAM,
            _: native::LPARAM,
        ) -> c_int {
            if msg == native::UCM_PROCESSDATA {
                -1
            } else {
                0
            }
        }
        for value in [Ok(()), Err(())] {
            let entry = Archive::new("data/version.rar")
                .open_for_processing()
                .unwrap()
                .read_header()
                .unwrap()
                .unwrap();
            // A stale callback would abort the unregistered process call below.
            let _ = unsafe {
                entry
                    .handle
                    .with_callback(abort_on_data, &mut (), |_| value)
            };
            assert_eq!(
                pathed::process_file(entry.handle.0.as_ptr(), native::RAR_TEST, None, None),
                native::ERAR_SUCCESS
            );
        }
    }
}
