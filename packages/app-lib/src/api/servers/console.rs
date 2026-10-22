//! Byte-preserving terminal history. Events wake readers; cursors recover missed events.

use std::collections::VecDeque;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use dashmap::DashMap;
use serde::Serialize;

use crate::state::LogCursor;

const CONSOLE_CAPACITY: usize = 4 * 1024 * 1024;
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);
static CONSOLES: LazyLock<DashMap<String, ConsoleBuffer>> =
    LazyLock::new(DashMap::new);

struct ConsoleBuffer {
    chunks: VecDeque<Vec<u8>>,
    byte_len: usize,
    epoch: u64,
    sequence: u64,
}

#[derive(Serialize)]
pub struct ConsoleDelta {
    cursor: LogCursor,
    reset: bool,
    data: String,
}

/// portable-pty enables ConPTY's INHERIT_CURSOR flag. Answer its initial DSR
/// even when no webview is attached, and omit it from replayable history.
pub(super) struct ConsoleHandshake {
    pending: Vec<u8>,
    active: bool,
}

impl ConsoleHandshake {
    pub(super) fn new(active: bool) -> Self {
        Self {
            pending: Vec::new(),
            active,
        }
    }

    pub(super) fn consume(&mut self, bytes: &[u8]) -> (Vec<u8>, bool) {
        if !self.active {
            return (bytes.to_vec(), false);
        }
        const QUERY: &[u8] = b"\x1b[6n";
        self.pending.extend_from_slice(bytes);
        if self.pending.len() < QUERY.len() && QUERY.starts_with(&self.pending)
        {
            return (Vec::new(), false);
        }
        self.active = false;
        let mut output = std::mem::take(&mut self.pending);
        let reply = output.starts_with(QUERY);
        if reply {
            output.drain(..QUERY.len());
        }
        (output, reply)
    }
}

impl ConsoleBuffer {
    fn new() -> Self {
        Self {
            chunks: VecDeque::new(),
            byte_len: 0,
            epoch: NEXT_EPOCH.fetch_add(1, Ordering::Relaxed),
            sequence: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        self.sequence += bytes.len() as u64;
        let bytes = &bytes[bytes.len().saturating_sub(CONSOLE_CAPACITY)..];
        while self.byte_len + bytes.len() > CONSOLE_CAPACITY {
            let Some(chunk) = self.chunks.pop_front() else {
                break;
            };
            self.byte_len -= chunk.len();
        }
        self.byte_len += bytes.len();
        self.chunks.push_back(bytes.to_vec());
    }

    fn delta(&self, cursor: Option<LogCursor>) -> ConsoleDelta {
        let first = self.sequence - self.byte_len as u64;
        let reset = cursor.is_none_or(|cursor| {
            cursor.epoch != self.epoch
                || cursor.sequence < first
                || cursor.sequence > self.sequence
        });
        let mut skip = if reset {
            0
        } else {
            (cursor.unwrap().sequence - first) as usize
        };
        let mut bytes = Vec::new();
        for chunk in &self.chunks {
            let skipped = skip.min(chunk.len());
            skip -= skipped;
            bytes.extend_from_slice(&chunk[skipped..]);
        }
        ConsoleDelta {
            cursor: LogCursor {
                epoch: self.epoch,
                sequence: self.sequence,
            },
            reset,
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }
}

pub(super) fn reset_console(server_id: &str) {
    CONSOLES.insert(server_id.to_string(), ConsoleBuffer::new());
}

pub(super) fn remove_console(server_id: &str) {
    CONSOLES.remove(server_id);
}

pub(super) async fn publish_console_output(server_id: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    CONSOLES
        .entry(server_id.to_string())
        .or_insert_with(ConsoleBuffer::new)
        .push(bytes);
    crate::event::emit::emit_server(
        server_id,
        crate::event::ServerPayloadType::ConsoleOutput {
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        },
    )
    .await
    .ok();
}

pub fn get_console_delta(
    server_id: &str,
    cursor: Option<LogCursor>,
) -> ConsoleDelta {
    CONSOLES
        .entry(server_id.to_string())
        .or_insert_with(ConsoleBuffer::new)
        .delta(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(delta: &ConsoleDelta) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(&delta.data)
            .unwrap()
    }

    #[test]
    fn conpty_startup_query_is_answered_once_even_when_split_across_reads() {
        let mut handshake = ConsoleHandshake::new(true);
        assert_eq!(handshake.consume(b"\x1b["), (vec![], false));
        assert_eq!(handshake.consume(b"6nhello"), (b"hello".to_vec(), true));
        assert_eq!(handshake.consume(b"\x1b[6n"), (b"\x1b[6n".to_vec(), false));
        let mut ordinary = ConsoleHandshake::new(true);
        assert_eq!(ordinary.consume(b"hello"), (b"hello".to_vec(), false));
        let mut unix = ConsoleHandshake::new(false);
        assert_eq!(unix.consume(b"\x1b[6n"), (b"\x1b[6n".to_vec(), false));
    }

    #[test]
    fn replays_raw_bytes_and_recovers_dropped_events_without_duplicates() {
        let mut buffer = ConsoleBuffer::new();
        buffer.push(b"\x1b[31m> \xe4\xb8");
        let first = buffer.delta(None);
        assert!(first.reset);
        assert_eq!(decode(&first), b"\x1b[31m> \xe4\xb8");
        buffer.push(b"\xad\x1b[0m\r\n");
        buffer.push(b"\x1b[?1049h");
        let delta = buffer.delta(Some(first.cursor));
        assert!(!delta.reset);
        assert_eq!(decode(&delta), b"\xad\x1b[0m\r\n\x1b[?1049h");
        assert!(decode(&buffer.delta(Some(delta.cursor))).is_empty());
    }

    #[test]
    fn bounds_history_and_resets_stale_or_previous_run_cursors() {
        let mut buffer = ConsoleBuffer::new();
        buffer.push(b"old");
        let old = buffer.delta(None).cursor;
        buffer.push(&vec![b'x'; CONSOLE_CAPACITY + 10]);
        assert_eq!(buffer.byte_len, CONSOLE_CAPACITY);
        let delta = buffer.delta(Some(old));
        assert!(delta.reset);
        assert_eq!(decode(&delta).len(), CONSOLE_CAPACITY);
        assert!(ConsoleBuffer::new().delta(Some(delta.cursor)).reset);
    }
}
