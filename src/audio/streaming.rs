use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Combined trait required by the rodio `Decoder` constructor.
///
/// `Decoder::new` accepts any `R: Read + Seek + Send + Sync + 'static`. This
/// supertrait lets us box that combination into a single `dyn AudioSource`.
pub trait AudioSource: Read + Seek + Send + Sync + 'static {}

impl<T: Read + Seek + Send + Sync + 'static> AudioSource for T {}

/// Boxed streaming audio reader accepted by the rodio decoder.
pub type AudioRaw = Box<dyn AudioSource>;

/// Internal buffer state shared between [`ProgressiveReader`] and [`ProgressiveWriter`].
struct BufState {
    data: Vec<u8>,
    done: bool,
    total_len: Option<u64>,
}

impl BufState {
    fn new(total_len: Option<u64>) -> Self {
        Self {
            data: Vec::new(),
            done: false,
            total_len,
        }
    }
}

/// The read side of a progressive download channel.
///
/// Blocks on [`Read::read`] while waiting for more bytes from the paired
/// [`ProgressiveWriter`], and unblocks when either more data arrives or the
/// writer signals completion. Returns `Ok(0)` (EOF) once the writer has called
/// [`ProgressiveWriter::finish`] and all buffered bytes have been consumed.
pub struct ProgressiveReader {
    state: Arc<(Mutex<BufState>, Condvar)>,
    /// Byte offset of the next read. `AtomicU64` makes the type `Sync` without
    /// requiring the mutex to be held during positional bookkeeping.
    pos: AtomicU64,
}

impl Read for ProgressiveReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let (lock, cvar) = &*self.state;
        let mut guard = lock.lock().map_err(|_| io::Error::other("lock poisoned"))?;
        loop {
            let pos = self.pos.load(Ordering::Relaxed) as usize;
            if pos < guard.data.len() {
                let available = &guard.data[pos..];
                let n = available.len().min(buf.len());
                buf[..n].copy_from_slice(&available[..n]);
                self.pos.fetch_add(n as u64, Ordering::Relaxed);
                return Ok(n);
            }
            if guard.done {
                // All bytes consumed and writer is finished — signal EOF.
                return Ok(0);
            }
            // Wait for the writer to push more data or call finish().
            guard = cvar
                .wait(guard)
                .map_err(|_| io::Error::other("lock poisoned"))?;
        }
    }
}

impl Seek for ProgressiveReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let (lock, _cvar) = &*self.state;

        let new_pos: u64 = match from {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(delta) => {
                let cur = self.pos.load(Ordering::Relaxed) as i64;
                let next = cur.checked_add(delta).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "seek would overflow")
                })?;
                if next < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "seek before start of stream",
                    ));
                }
                next as u64
            }
            SeekFrom::End(delta) => {
                // Use Content-Length when available to avoid blocking.
                let total = {
                    let guard = lock.lock().map_err(|_| io::Error::other("lock poisoned"))?;
                    guard.total_len
                };
                let total = total.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        "SeekFrom::End requires Content-Length",
                    )
                })?;
                let end = total as i64;
                let next = end.checked_add(delta).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "seek would overflow")
                })?;
                if next < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "seek before start of stream",
                    ));
                }
                next as u64
            }
        };

        self.pos.store(new_pos, Ordering::Relaxed);
        Ok(new_pos)
    }
}

/// The write side of a progressive download channel.
///
/// The download task calls [`Self::push`] for each chunk and [`Self::finish`]
/// when the download is complete (or has failed). Both operations wake the
/// paired [`ProgressiveReader`].
pub struct ProgressiveWriter {
    state: Arc<(Mutex<BufState>, Condvar)>,
}

impl ProgressiveWriter {
    /// Append `chunk` to the shared buffer and wake the reader.
    pub fn push(&self, chunk: &[u8]) {
        let (lock, cvar) = &*self.state;
        if let Ok(mut guard) = lock.lock() {
            guard.data.extend_from_slice(chunk);
            cvar.notify_one();
        }
    }

    /// Signal that no more bytes will be written, unblocking the reader
    /// permanently once it has consumed all buffered bytes.
    pub fn finish(self) {
        let (lock, cvar) = &*self.state;
        if let Ok(mut guard) = lock.lock() {
            guard.done = true;
            cvar.notify_all();
        }
    }
}

/// Create a linked `(reader, writer)` pair.
///
/// `total_len` should be set to the `Content-Length` of the HTTP response when
/// available; it is used by [`Seek::seek`] with [`SeekFrom::End`] to avoid
/// blocking the decoder.
#[must_use]
pub fn progressive_channel(total_len: Option<u64>) -> (ProgressiveReader, ProgressiveWriter) {
    let state = Arc::new((Mutex::new(BufState::new(total_len)), Condvar::new()));
    let reader = ProgressiveReader {
        state: Arc::clone(&state),
        pos: AtomicU64::new(0),
    };
    let writer = ProgressiveWriter { state };
    (reader, writer)
}
