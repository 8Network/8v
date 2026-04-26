//! Pipe capture and drain — concurrent output collection with timeout.
//!
//! Capture uses a shared buffer (`Arc<Mutex<Vec<u8>>>`) so that if the drain
//! thread hangs (setsid descendant holding pipe open), the already-captured
//! bytes are still retrievable via `collect_pair`.

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Find the last valid UTF-8 character boundary at or before `max` bytes.
///
/// Walks backwards from `max`, skipping UTF-8 continuation bytes (0x80..0xBF),
/// to ensure truncation doesn't split a multibyte character.
fn floor_char_boundary(bytes: &[u8], max: usize) -> usize {
    if max >= bytes.len() {
        return bytes.len();
    }
    let mut i = max;
    // Walk backwards past UTF-8 continuation bytes (10xxxxxx).
    // A continuation byte has high bits 10.
    while i > 0 && bytes[i] & 0xC0 == 0x80 {
        i -= 1;
    }
    i
}

/// Captured output from one stream.
pub(crate) struct CapturedStream {
    pub data: Vec<u8>,
    pub truncated: bool,
}

/// Captured output from both stdout and stderr streams.
pub(crate) struct CapturedPair {
    pub stdout: CapturedStream,
    pub stderr: CapturedStream,
}

/// Shared capture result — written by the drain thread, readable by the collector.
pub(crate) struct CaptureHandle {
    /// The drain thread.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared buffer — capture thread writes here, collector reads on timeout.
    pub buf: Arc<Mutex<Vec<u8>>>,
    /// Whether the stream was truncated (more data than max_bytes).
    pub truncated: Arc<Mutex<bool>>,
}

/// Spawn a capture thread that writes to a shared buffer.
///
/// Phase 1: read up to `max_bytes + 1` into the shared buffer.
/// Phase 2: drain remaining to /dev/null (prevents SIGPIPE).
///
/// The shared buffer is readable even if the thread hangs in Phase 2.
pub(crate) fn spawn_capture(
    stream: Option<impl Read + Send + 'static>,
    max_bytes: usize,
) -> CaptureHandle {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let truncated = Arc::new(Mutex::new(false));

    let buf_clone = Arc::clone(&buf);
    let truncated_clone = Arc::clone(&truncated);

    let handle = std::thread::spawn(move || {
        let Some(mut stream) = stream else {
            return;
        };

        // Phase 1: capture up to max_bytes + 1 (the +1 detects truncation).
        // Saturate to avoid overflow when max_bytes = usize::MAX.
        let read_limit = max_bytes.saturating_add(1);
        let mut local_buf = Vec::new();
        if let Err(e) = (&mut stream)
            .take(read_limit as u64)
            .read_to_end(&mut local_buf)
        {
            tracing::warn!(error = %e, "pipe read failed — output may be incomplete");
        }

        let was_truncated = local_buf.len() > max_bytes;
        if was_truncated {
            // Truncate at a valid UTF-8 boundary to avoid splitting multibyte chars.
            let boundary = floor_char_boundary(&local_buf, max_bytes);
            local_buf.truncate(boundary);
        }

        // Write captured data to shared buffer BEFORE draining.
        // If the drain hangs, the collector can still read this.
        // On panic, recover data from poisoned mutex rather than losing it.
        {
            let mut guard = match buf_clone.lock() {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("buf mutex poisoned — recovering captured data");
                    e.into_inner()
                }
            };
            *guard = local_buf;
        }
        {
            let mut guard = match truncated_clone.lock() {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("truncated mutex poisoned — recovering truncation flag");
                    e.into_inner()
                }
            };
            *guard = was_truncated;
        }

        // Phase 2: drain remaining to /dev/null so the child doesn't get SIGPIPE.
        let mut discard = [0u8; 8192];
        while let Ok(n) = stream.read(&mut discard) {
            if n == 0 {
                break;
            }
        }
    });

    CaptureHandle {
        handle,
        buf,
        truncated,
    }
}

/// Collect both streams with a SHARED wall-clock deadline.
/// Both joiner threads start concurrently — neither starves the other.
///
/// If a thread hangs (setsid descendant), the already-captured bytes are
/// extracted from the shared buffer. Only the drain phase is lost, not the data.
pub(crate) fn collect_pair(
    stdout: CaptureHandle,
    stderr: CaptureHandle,
    timeout: Duration,
) -> CapturedPair {
    let deadline = std::time::Instant::now() + timeout;

    // Clone shared buffer Arcs BEFORE moving handles into joiner threads.
    // On timeout, we can still extract captured data from the shared buffers.
    let stdout_buf = Arc::clone(&stdout.buf);
    let stdout_trunc = Arc::clone(&stdout.truncated);
    let stderr_buf = Arc::clone(&stderr.buf);
    let stderr_trunc = Arc::clone(&stderr.truncated);

    // Spawn both joiner threads up front so both run concurrently.
    let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
    let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let _ = stdout_tx.send(stdout.handle.join());
    });
    std::thread::spawn(move || {
        let _ = stderr_tx.send(stderr.handle.join());
    });

    let stdout_result =
        collect_from_channel(stdout_rx, &stdout_buf, &stdout_trunc, deadline, "stdout");
    let stderr_result =
        collect_from_channel(stderr_rx, &stderr_buf, &stderr_trunc, deadline, "stderr");

    CapturedPair {
        stdout: stdout_result,
        stderr: stderr_result,
    }
}

/// Extract data from a joiner channel against a shared deadline.
fn collect_from_channel(
    rx: std::sync::mpsc::Receiver<Result<(), Box<dyn std::any::Any + Send>>>,
    buf_arc: &Arc<Mutex<Vec<u8>>>,
    trunc_arc: &Arc<Mutex<bool>>,
    deadline: std::time::Instant,
    stream_name: &str,
) -> CapturedStream {
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());

    let extract = || {
        // On mutex poison, recover the data rather than returning empty.
        // The data is still valid even if the thread panicked.
        let data = match buf_arc.lock() {
            Ok(g) => g.clone(),
            Err(e) => {
                tracing::warn!(
                    stream = stream_name,
                    "buf mutex poisoned — recovering captured data"
                );
                e.into_inner().clone()
            }
        };
        let truncated = match trunc_arc.lock() {
            Ok(g) => *g,
            Err(e) => {
                tracing::warn!(
                    stream = stream_name,
                    "truncated mutex poisoned — recovering flag"
                );
                *e.into_inner()
            }
        };
        CapturedStream { data, truncated }
    };

    match rx.recv_timeout(remaining) {
        Ok(Ok(())) => extract(),
        Ok(Err(_)) => {
            tracing::warn!(
                stream = stream_name,
                "drain thread panicked — extracting captured data"
            );
            extract()
        }
        Err(_) => {
            tracing::warn!(
                stream = stream_name,
                "drain thread did not finish — extracting captured data from shared buffer"
            );
            extract()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn floor_char_boundary_empty() {
        let bytes = b"";
        assert_eq!(floor_char_boundary(bytes, 0), 0);
        assert_eq!(floor_char_boundary(bytes, 100), 0);
    }

    #[test]
    fn floor_char_boundary_ascii_only() {
        let bytes = b"hello";
        assert_eq!(floor_char_boundary(bytes, 5), 5);
        assert_eq!(floor_char_boundary(bytes, 3), 3);
        assert_eq!(floor_char_boundary(bytes, 0), 0);
    }

    #[test]
    fn floor_char_boundary_multibyte_not_split() {
        // "🦀" is the crab emoji, 4 bytes: F0 9F A6 80
        let crab = "🦀";
        let bytes = crab.as_bytes();
        assert_eq!(bytes.len(), 4);

        // If we try to truncate in the middle (e.g., at byte 2), we should get 0
        // because we can't safely include a partial character.
        assert_eq!(floor_char_boundary(bytes, 2), 0);
        assert_eq!(floor_char_boundary(bytes, 3), 0);

        // Truncating at or after the full character is safe.
        assert_eq!(floor_char_boundary(bytes, 4), 4);
    }

    #[test]
    fn floor_char_boundary_multibyte_string() {
        // "hello🦀world" contains a 4-byte emoji in the middle
        let s = "hello🦀world";
        let bytes = s.as_bytes();

        // Truncate at position 5 (end of "hello") is safe.
        assert_eq!(floor_char_boundary(bytes, 5), 5);

        // Truncate at positions 6, 7, 8 (inside the emoji) should back up to 5.
        assert_eq!(floor_char_boundary(bytes, 6), 5);
        assert_eq!(floor_char_boundary(bytes, 7), 5);
        assert_eq!(floor_char_boundary(bytes, 8), 5);

        // Truncate at position 9 (after emoji) is safe.
        assert_eq!(floor_char_boundary(bytes, 9), 9);

        // Truncate beyond the string is safe.
        assert_eq!(floor_char_boundary(bytes, 100), bytes.len());
    }

    #[test]
    fn spawn_capture_truncates_at_valid_utf8_boundary() {
        // Create a simple reader that outputs a string with a multibyte char.
        // We'll test that truncation at max_bytes doesn't split the emoji.
        let output = "hello🦀world";
        let cursor = Cursor::new(output.as_bytes().to_vec());

        // Set max_bytes to 6, which is inside the 4-byte emoji (starts at byte 5).
        // This would split the emoji if we naively truncated at byte 6.
        let max_bytes = 6;
        let handle = spawn_capture(Some(cursor), max_bytes);

        // Give the capture thread time to finish.
        let _ = handle.handle.join();

        let buf_guard = handle.buf.lock().unwrap();
        let truncated_guard = handle.truncated.lock().unwrap();

        // The output should be truncated.
        assert!(*truncated_guard);

        // The truncated output should be valid UTF-8: "hello" (5 bytes).
        let result = String::from_utf8(buf_guard.clone());
        assert!(result.is_ok(), "Truncated output must be valid UTF-8");

        let truncated_str = result.unwrap();
        assert_eq!(truncated_str, "hello");
    }

    #[test]
    fn spawn_capture_preserves_output_when_under_limit() {
        let output = "hello";
        let cursor = Cursor::new(output.as_bytes().to_vec());

        // max_bytes is larger than the output.
        let max_bytes = 100;
        let handle = spawn_capture(Some(cursor), max_bytes);

        let _ = handle.handle.join();

        let buf_guard = handle.buf.lock().unwrap();
        let truncated_guard = handle.truncated.lock().unwrap();

        // Should not be truncated.
        assert!(!*truncated_guard);

        // All output should be present.
        assert_eq!(String::from_utf8(buf_guard.clone()).unwrap(), "hello");
    }

    #[test]
    fn mutex_poison_recovery_preserves_data() {
        // Verify that poisoned mutex recovery actually works.
        // When a thread panics while holding a mutex lock, the mutex becomes poisoned.
        // We must be able to recover the data using unwrap_or_else(|e| e.into_inner()).
        let buf = Arc::new(Mutex::new(vec![1u8, 2, 3, 4, 5]));
        let buf_clone = Arc::clone(&buf);

        // Spawn a thread that panics while holding the lock.
        let join_handle = std::thread::spawn(move || {
            let _guard = buf_clone.lock().unwrap();
            panic!("intentional panic to poison mutex");
        });

        // Wait for the thread to panic (this will return Err because of the panic).
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = join_handle.join();
        }));

        // The mutex is now poisoned. Verify we can still recover the data.
        let recovered = match buf.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => e.into_inner().clone(),
        };

        // The data is preserved despite the poison.
        assert_eq!(recovered, vec![1u8, 2, 3, 4, 5]);
    }
}
