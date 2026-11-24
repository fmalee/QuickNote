use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::error::{AppError, ErrorCode};

/// Maximum bytes retained in a hidden terminal's ring buffer (1 MiB, `C8.7`).
pub const TERMINAL_RING_BUFFER_MAX_BYTES: usize = 1024 * 1024;

/// Cumulative unconsumed output threshold triggering backpressure pause (10 MiB, `v4` §5.6.7).
pub const TERMINAL_BACKPRESSURE_LIMIT_BYTES: usize = 10 * 1024 * 1024;

/// Output flush interval for streaming PTY chunks to frontend (`C4.3`).
pub const TERMINAL_FLUSH_INTERVAL_MS: u64 = 16;

/// Output batch size cap per flush chunk (64 KiB, `C4.3`).
pub const TERMINAL_FLUSH_BATCH_BYTES: usize = 64 * 1024;

/// Background idle sweep period in seconds (`C8.7`).
pub const TERMINAL_IDLE_SWEEP_INTERVAL_SECS: u64 = 60;

/// Lifecycle state of an embedded terminal session (`C8.7`, `D-0005`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum TerminalLifecycleState {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "hidden_alive")]
    HiddenAlive,
    #[serde(rename = "recycled")]
    Recycled,
}

/// Reason a terminal session exited or was recycled (`C4.2`, `C8.7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum TerminalExitReason {
    #[serde(rename = "process_exit")]
    ProcessExit,
    #[serde(rename = "manual_close")]
    ManualClose,
    #[serde(rename = "idle_recycled")]
    IdleRecycled,
    #[serde(rename = "app_exit")]
    AppExit,
}

/// Arguments for spawning a workspace-scoped terminal session (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSpawnArgs {
    pub workspace_key: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub shell: Option<String>,
}

/// Arguments for writing base64-encoded input bytes into a terminal session (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalWriteArgs {
    pub session_id: String,
    pub data_base64: String,
}

/// Arguments for resizing a terminal PTY (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResizeArgs {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

/// Arguments for toggling terminal visibility / fold-keepalive (`C4.2`, `C8.7`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalVisibilityArgs {
    pub session_id: String,
    pub visible: bool,
}

/// Arguments for manually killing a terminal session (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalKillArgs {
    pub session_id: String,
}

/// Snapshot metadata of a terminal session (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSessionInfo {
    pub session_id: String,
    pub workspace_key: String,
    pub cwd: String,
    pub has_active_foreground_child: bool,
    pub output_paused: bool,
    pub state: TerminalLifecycleState,
}

/// Streamed PTY output chunk sent over `tauri::ipc::Channel` (`C4.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalData {
    pub session_id: String,
    pub data_base64: String,
}

/// Event payload emitted when a terminal session terminates (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExitPayload {
    pub session_id: String,
    pub exit_code: Option<i32>,
    pub reason: TerminalExitReason,
}

/// Fixed-capacity byte ring buffer retaining the newest `max_bytes` when hidden (`C8.7`).
#[derive(Debug, Clone)]
pub struct TerminalRingBuffer {
    max_bytes: usize,
    buf: VecDeque<u8>,
}

impl Default for TerminalRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalRingBuffer {
    /// Creates a ring buffer with the default 1 MiB cap (`TERMINAL_RING_BUFFER_MAX_BYTES`).
    pub fn new() -> Self {
        Self::with_capacity(TERMINAL_RING_BUFFER_MAX_BYTES)
    }

    /// Creates a ring buffer with an explicit maximum byte capacity.
    pub fn with_capacity(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            buf: VecDeque::with_capacity(max_bytes.min(64 * 1024)),
        }
    }

    /// Appends `bytes`, evicting the oldest bytes if capacity is exceeded.
    pub fn push(&mut self, bytes: &[u8]) {
        if self.max_bytes == 0 || bytes.is_empty() {
            return;
        }
        if bytes.len() >= self.max_bytes {
            self.buf.clear();
            let start = bytes.len() - self.max_bytes;
            self.buf.extend(&bytes[start..]);
            return;
        }
        let total = self.buf.len() + bytes.len();
        if total > self.max_bytes {
            let overflow = total - self.max_bytes;
            self.buf.drain(..overflow);
        }
        self.buf.extend(bytes);
    }

    /// Drains all buffered bytes in FIFO order and resets the buffer to empty.
    pub fn drain_all(&mut self) -> Vec<u8> {
        self.buf.drain(..).collect()
    }

    /// Clears all buffered bytes.
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// Returns the number of bytes currently stored.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` if no bytes are stored.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

/// Evaluates whether a terminal session qualifies for idle recycling (`C8.7`, `D-0005`).
pub fn should_idle_recycle(
    state: TerminalLifecycleState,
    idle_recycle_minutes: u32,
    has_active_foreground_child: bool,
    last_io_at_ms: i64,
    now_ms: i64,
) -> bool {
    if state != TerminalLifecycleState::HiddenAlive {
        return false;
    }
    if idle_recycle_minutes == 0 || has_active_foreground_child {
        return false;
    }
    let threshold_ms = i64::from(idle_recycle_minutes).saturating_mul(60_000);
    now_ms.saturating_sub(last_io_at_ms) >= threshold_ms
}

const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes raw bytes using standard RFC 4648 base64 (`+`, `/`, `=`).
pub fn encode_base64(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };

        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(B64_ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 0x3f) as usize] as char);

        if i + 1 < bytes.len() {
            out.push(B64_ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }

        if i + 2 < bytes.len() {
            out.push(B64_ALPHABET[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }

        i += 3;
    }
    out
}

fn decode_b64_val(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodes standard RFC 4648 base64, rejecting invalid characters, padding, or lengths.
pub fn decode_base64(raw: &str) -> Result<Vec<u8>, AppError> {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.len().is_multiple_of(4) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Invalid base64 length",
        ));
    }

    let mut out = Vec::with_capacity((bytes.len() / 4) * 3);
    let mut i = 0;
    while i < bytes.len() {
        let c0 = bytes[i];
        let c1 = bytes[i + 1];
        let c2 = bytes[i + 2];
        let c3 = bytes[i + 3];

        let v0 = decode_b64_val(c0)
            .ok_or_else(|| AppError::new(ErrorCode::ValidationError, "Invalid base64 character"))?;
        let v1 = decode_b64_val(c1)
            .ok_or_else(|| AppError::new(ErrorCode::ValidationError, "Invalid base64 character"))?;

        let is_last_block = i + 4 == bytes.len();
        if c2 == b'=' {
            if !is_last_block || c3 != b'=' {
                return Err(AppError::new(
                    ErrorCode::ValidationError,
                    "Invalid base64 padding",
                ));
            }
            if (v1 & 0x0f) != 0 {
                return Err(AppError::new(
                    ErrorCode::ValidationError,
                    "Invalid non-zero base64 padding bits",
                ));
            }
            out.push((v0 << 2) | (v1 >> 4));
        } else if c3 == b'=' {
            if !is_last_block {
                return Err(AppError::new(
                    ErrorCode::ValidationError,
                    "Invalid base64 padding",
                ));
            }
            let v2 = decode_b64_val(c2).ok_or_else(|| {
                AppError::new(ErrorCode::ValidationError, "Invalid base64 character")
            })?;
            if (v2 & 0x03) != 0 {
                return Err(AppError::new(
                    ErrorCode::ValidationError,
                    "Invalid non-zero base64 padding bits",
                ));
            }
            out.push((v0 << 2) | (v1 >> 4));
            out.push((v1 << 4) | (v2 >> 2));
        } else {
            let v2 = decode_b64_val(c2).ok_or_else(|| {
                AppError::new(ErrorCode::ValidationError, "Invalid base64 character")
            })?;
            let v3 = decode_b64_val(c3).ok_or_else(|| {
                AppError::new(ErrorCode::ValidationError, "Invalid base64 character")
            })?;
            out.push((v0 << 2) | (v1 >> 4));
            out.push((v1 << 4) | (v2 >> 2));
            out.push((v2 << 6) | v3);
        }

        i += 4;
    }

    Ok(out)
}
