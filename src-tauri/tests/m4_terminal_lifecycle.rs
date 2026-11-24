use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::application::terminal::{TerminalDataSink, TerminalEventSink, TerminalManager};
use quicknote_lib::domain::error::{AppError, ErrorCode};
use quicknote_lib::domain::settings::{
    PanelsSettingsPatch, SettingsPatch, TerminalPanelSettingsPatch,
};
use quicknote_lib::domain::terminal::{
    decode_base64, encode_base64, should_idle_recycle, TerminalData, TerminalExitPayload,
    TerminalExitReason, TerminalKillArgs, TerminalLifecycleState, TerminalResizeArgs,
    TerminalRingBuffer, TerminalSessionInfo, TerminalSpawnArgs, TerminalVisibilityArgs,
    TerminalWriteArgs, TERMINAL_BACKPRESSURE_LIMIT_BYTES, TERMINAL_RING_BUFFER_MAX_BYTES,
};
use quicknote_lib::infra::pty::{PortablePtyBackend, PtyBackend, PtySessionHandle, PtySpawnConfig};

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "quicknote-m4-01a-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

struct RecordingDataSink {
    chunks: Mutex<Vec<TerminalData>>,
}

impl RecordingDataSink {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            chunks: Mutex::new(Vec::new()),
        })
    }

    fn all_decoded_bytes(&self) -> Vec<u8> {
        let guard = self.chunks.lock().expect("lock chunks");
        let mut out = Vec::new();
        for chunk in guard.iter() {
            if let Ok(decoded) = decode_base64(&chunk.data_base64) {
                out.extend_from_slice(&decoded);
            }
        }
        out
    }

    fn clear(&self) {
        self.chunks.lock().expect("lock chunks").clear();
    }
}

impl TerminalDataSink for RecordingDataSink {
    fn send(&self, data: TerminalData) -> Result<(), AppError> {
        self.chunks.lock().expect("lock chunks").push(data);
        Ok(())
    }
}

struct RecordingEventSink {
    states: Mutex<Vec<TerminalSessionInfo>>,
    exits: Mutex<Vec<TerminalExitPayload>>,
}

impl RecordingEventSink {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            states: Mutex::new(Vec::new()),
            exits: Mutex::new(Vec::new()),
        })
    }

    fn states(&self) -> Vec<TerminalSessionInfo> {
        self.states.lock().expect("lock states").clone()
    }

    fn exits(&self) -> Vec<TerminalExitPayload> {
        self.exits.lock().expect("lock exits").clone()
    }
}

impl TerminalEventSink for RecordingEventSink {
    fn emit_session_state(&self, info: &TerminalSessionInfo) {
        self.states.lock().expect("lock states").push(info.clone());
    }

    fn emit_exit(&self, payload: &TerminalExitPayload) {
        self.exits.lock().expect("lock exits").push(payload.clone());
    }
}

struct ChannelReader {
    rx: mpsc::Receiver<Vec<u8>>,
    buf: Vec<u8>,
    pos: usize,
}

impl Read for ChannelReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.buf.len() {
            match self.rx.recv() {
                Ok(chunk) => {
                    self.buf = chunk;
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let avail = self.buf.len() - self.pos;
        let to_copy = avail.min(out.len());
        out[..to_copy].copy_from_slice(&self.buf[self.pos..self.pos + to_copy]);
        self.pos += to_copy;
        Ok(to_copy)
    }
}

struct MockPtyHandle {
    tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    reader: Mutex<Option<Box<dyn Read + Send>>>,
    has_fg: AtomicBool,
    terminated: AtomicBool,
}

impl MockPtyHandle {
    fn new() -> Arc<Self> {
        let (tx, rx) = mpsc::channel();
        Arc::new(Self {
            tx: Mutex::new(Some(tx)),
            reader: Mutex::new(Some(Box::new(ChannelReader {
                rx,
                buf: Vec::new(),
                pos: 0,
            }))),
            has_fg: AtomicBool::new(false),
            terminated: AtomicBool::new(false),
        })
    }

    fn push_output(&self, bytes: &[u8]) {
        if let Some(tx) = self.tx.lock().expect("lock tx").as_ref() {
            let _ = tx.send(bytes.to_vec());
        }
    }

    fn set_foreground_child(&self, active: bool) {
        self.has_fg.store(active, Ordering::Relaxed);
    }
}

impl PtySessionHandle for MockPtyHandle {
    fn take_reader(&self) -> Result<Box<dyn Read + Send>, AppError> {
        self.reader
            .lock()
            .expect("lock reader")
            .take()
            .ok_or_else(|| AppError::new(ErrorCode::InternalError, "reader already taken"))
    }

    fn write_all(&self, _data: &[u8]) -> Result<(), AppError> {
        Ok(())
    }

    fn resize(&self, _cols: u16, _rows: u16) -> Result<(), AppError> {
        Ok(())
    }

    fn has_active_foreground_child(&self) -> bool {
        self.has_fg.load(Ordering::Relaxed)
    }

    fn try_wait(&self) -> Result<Option<i32>, AppError> {
        if self.terminated.load(Ordering::Relaxed) {
            Ok(Some(0))
        } else {
            Ok(None)
        }
    }

    fn terminate_process_group(&self) -> Result<Option<i32>, AppError> {
        self.terminated.store(true, Ordering::Relaxed);
        *self.tx.lock().expect("lock tx") = None;
        Ok(Some(0))
    }
}

struct MockPtyBackend {
    handles: Mutex<Vec<Arc<MockPtyHandle>>>,
}

impl MockPtyBackend {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            handles: Mutex::new(Vec::new()),
        })
    }

    fn last_handle(&self) -> Arc<MockPtyHandle> {
        self.handles
            .lock()
            .expect("lock handles")
            .last()
            .cloned()
            .expect("at least one handle")
    }
}

impl PtyBackend for MockPtyBackend {
    fn spawn(&self, _config: &PtySpawnConfig) -> Result<Arc<dyn PtySessionHandle>, AppError> {
        let handle = MockPtyHandle::new();
        self.handles
            .lock()
            .expect("lock handles")
            .push(Arc::clone(&handle));
        Ok(handle)
    }
}

#[test]
fn ring_buffer_retains_latest_1mb_and_drains() {
    let mut rb = TerminalRingBuffer::new();
    let chunk_a = vec![b'A'; 700 * 1024];
    let chunk_b = vec![b'B'; 600 * 1024];

    rb.push(&chunk_a);
    rb.push(&chunk_b);

    assert_eq!(rb.len(), TERMINAL_RING_BUFFER_MAX_BYTES);
    let drained = rb.drain_all();
    assert_eq!(drained.len(), TERMINAL_RING_BUFFER_MAX_BYTES);
    assert!(rb.is_empty());

    let expected_a_count = TERMINAL_RING_BUFFER_MAX_BYTES - chunk_b.len();
    assert!(drained[..expected_a_count].iter().all(|&b| b == b'A'));
    assert!(drained[expected_a_count..].iter().all(|&b| b == b'B'));

    let huge = vec![b'C'; TERMINAL_RING_BUFFER_MAX_BYTES + 123];
    rb.push(&huge);
    assert_eq!(rb.len(), TERMINAL_RING_BUFFER_MAX_BYTES);
    rb.clear();
    assert!(rb.is_empty());
}

#[test]
fn base64_roundtrip_and_validation() {
    for sample in [b"".as_slice(), b"f", b"fo", b"foo", b"hello\nworld\x00\xff"] {
        let enc = encode_base64(sample);
        let dec = decode_base64(&enc).expect("valid base64");
        assert_eq!(dec, sample);
    }

    for invalid in ["abc", "ab=d", "a===", "!!!!", "ab=="] {
        let err = decode_base64(invalid).expect_err("must reject invalid base64");
        assert_eq!(err.code, ErrorCode::ValidationError);
    }
}

#[test]
fn real_pty_spawn_write_resize_and_kill() {
    let app_dir = temp_dir("real-pty");
    let work_dir = temp_dir("real-pty-cwd");
    let pm = Arc::new(PersistenceManager::open(app_dir.clone()).expect("open pm"));
    let manager = TerminalManager::with_backend_and_interval(
        Arc::new(PortablePtyBackend::new()),
        pm,
        Duration::from_secs(60),
    );
    let events = RecordingEventSink::new();
    manager
        .set_event_sink(events.clone())
        .expect("set event sink");

    let sink = RecordingDataSink::new();
    let info = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-real".to_string(),
                cwd: work_dir.to_string_lossy().to_string(),
                cols: 80,
                rows: 24,
                shell: Some("/bin/sh".to_string()),
            },
            sink.clone(),
        )
        .expect("spawn real pty");
    assert_eq!(info.state, TerminalLifecycleState::Active);

    manager
        .write_session(TerminalWriteArgs {
            session_id: info.session_id.clone(),
            data_base64: encode_base64(b"echo QUICKNOTE_PTY_OK\n"),
        })
        .expect("write to pty");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while Instant::now() < deadline {
        let text = String::from_utf8_lossy(&sink.all_decoded_bytes()).to_string();
        if text.contains("QUICKNOTE_PTY_OK") {
            found = true;
            break;
        }
        thread::sleep(Duration::from_millis(30));
    }
    assert!(found, "expected PTY output to contain QUICKNOTE_PTY_OK");

    manager
        .resize_session(TerminalResizeArgs {
            session_id: info.session_id.clone(),
            cols: 100,
            rows: 30,
        })
        .expect("resize pty");

    manager
        .kill_session(TerminalKillArgs {
            session_id: info.session_id.clone(),
        })
        .expect("kill pty");

    assert!(manager.list_sessions().expect("list").is_empty());
    let exits = events.exits();
    assert!(exits
        .iter()
        .any(|e| e.session_id == info.session_id && e.reason == TerminalExitReason::ManualClose));

    let _ = std::fs::remove_dir_all(&app_dir);
    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn concurrency_limits_per_workspace_and_global_three() {
    let app_dir = temp_dir("limits");
    let work_dir = temp_dir("limits-cwd");
    let pm = Arc::new(PersistenceManager::open(app_dir.clone()).expect("open pm"));
    let backend = MockPtyBackend::new();
    let manager =
        TerminalManager::with_backend_and_interval(backend.clone(), pm, Duration::from_secs(60));

    let cwd = work_dir.to_string_lossy().to_string();
    let s1 = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-1".to_string(),
                cwd: cwd.clone(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            RecordingDataSink::new(),
        )
        .expect("spawn ws-1");

    let dup_err = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-1".to_string(),
                cwd: cwd.clone(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            RecordingDataSink::new(),
        )
        .expect_err("duplicate workspace must fail");
    assert_eq!(dup_err.code, ErrorCode::TerminalLimitReached);

    let _s2 = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-2".to_string(),
                cwd: cwd.clone(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            RecordingDataSink::new(),
        )
        .expect("spawn ws-2");
    let _s3 = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-3".to_string(),
                cwd: cwd.clone(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            RecordingDataSink::new(),
        )
        .expect("spawn ws-3");

    let fourth_err = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-4".to_string(),
                cwd,
                cols: 80,
                rows: 24,
                shell: None,
            },
            RecordingDataSink::new(),
        )
        .expect_err("4th session must hit limit");
    assert_eq!(fourth_err.code, ErrorCode::TerminalLimitReached);

    manager
        .kill_session(TerminalKillArgs {
            session_id: s1.session_id,
        })
        .expect("kill s1");
    manager.shutdown_all();

    let _ = std::fs::remove_dir_all(&app_dir);
    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn fold_keepalive_buffers_while_hidden_and_flushes_on_visible() {
    let app_dir = temp_dir("fold");
    let work_dir = temp_dir("fold-cwd");
    let pm = Arc::new(PersistenceManager::open(app_dir.clone()).expect("open pm"));
    let backend = MockPtyBackend::new();
    let manager =
        TerminalManager::with_backend_and_interval(backend.clone(), pm, Duration::from_secs(60));
    let sink = RecordingDataSink::new();

    let info = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-fold".to_string(),
                cwd: work_dir.to_string_lossy().to_string(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            sink.clone(),
        )
        .expect("spawn");
    let handle = backend.last_handle();

    manager
        .set_visibility(TerminalVisibilityArgs {
            session_id: info.session_id.clone(),
            visible: false,
        })
        .expect("hide");
    sink.clear();

    handle.push_output(b"hidden-line-1\n");
    handle.push_output(b"hidden-line-2\n");
    thread::sleep(Duration::from_millis(60));
    assert!(
        sink.all_decoded_bytes().is_empty(),
        "must not push to frontend while hidden"
    );

    manager
        .set_visibility(TerminalVisibilityArgs {
            session_id: info.session_id,
            visible: true,
        })
        .expect("show");

    let flushed = String::from_utf8_lossy(&sink.all_decoded_bytes()).to_string();
    assert!(flushed.contains("hidden-line-1"));
    assert!(flushed.contains("hidden-line-2"));

    manager.shutdown_all();
    let _ = std::fs::remove_dir_all(&app_dir);
    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn backpressure_pauses_at_10mb_and_resumes_on_visibility_reset() {
    let app_dir = temp_dir("backpressure");
    let work_dir = temp_dir("backpressure-cwd");
    let pm = Arc::new(PersistenceManager::open(app_dir.clone()).expect("open pm"));
    let backend = MockPtyBackend::new();
    let manager =
        TerminalManager::with_backend_and_interval(backend.clone(), pm, Duration::from_secs(60));
    let events = RecordingEventSink::new();
    manager.set_event_sink(events.clone()).expect("sink");
    let sink = RecordingDataSink::new();

    let info = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-bp".to_string(),
                cwd: work_dir.to_string_lossy().to_string(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            sink.clone(),
        )
        .expect("spawn");
    let handle = backend.last_handle();

    let flood = vec![b'X'; TERMINAL_BACKPRESSURE_LIMIT_BYTES + 256 * 1024];
    for chunk in flood.chunks(256 * 1024) {
        handle.push_output(chunk);
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut paused = false;
    while Instant::now() < deadline {
        let list = manager.list_sessions().expect("list");
        if list
            .iter()
            .any(|s| s.session_id == info.session_id && s.output_paused)
        {
            paused = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(paused, "expected output_paused=true after >10MB flood");
    assert!(events
        .states()
        .iter()
        .any(|s| s.session_id == info.session_id && s.output_paused));

    let bytes_before_extra = sink.all_decoded_bytes().len();
    handle.push_output(b"dropped-while-paused");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(sink.all_decoded_bytes().len(), bytes_before_extra);

    manager
        .set_visibility(TerminalVisibilityArgs {
            session_id: info.session_id.clone(),
            visible: true,
        })
        .expect("resume");

    let list_after = manager.list_sessions().expect("list");
    assert!(list_after
        .iter()
        .any(|s| s.session_id == info.session_id && !s.output_paused));

    handle.push_output(b"after-resume-ok");
    let deadline2 = Instant::now() + Duration::from_secs(2);
    let mut resumed_output = false;
    while Instant::now() < deadline2 {
        let text = String::from_utf8_lossy(&sink.all_decoded_bytes()).to_string();
        if text.contains("after-resume-ok") {
            resumed_output = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(resumed_output, "expected output to flow again after resume");

    manager.shutdown_all();
    let _ = std::fs::remove_dir_all(&app_dir);
    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn idle_recycle_rules_and_exemptions() {
    assert!(!should_idle_recycle(
        TerminalLifecycleState::Active,
        30,
        false,
        0,
        31 * 60_000
    ));
    assert!(!should_idle_recycle(
        TerminalLifecycleState::HiddenAlive,
        0,
        false,
        0,
        120 * 60_000
    ));
    assert!(!should_idle_recycle(
        TerminalLifecycleState::HiddenAlive,
        30,
        true,
        0,
        120 * 60_000
    ));
    assert!(should_idle_recycle(
        TerminalLifecycleState::HiddenAlive,
        30,
        false,
        1000,
        1000 + 30 * 60_000
    ));

    let app_dir = temp_dir("idle");
    let work_dir = temp_dir("idle-cwd");
    let pm = Arc::new(PersistenceManager::open(app_dir.clone()).expect("open pm"));
    let backend = MockPtyBackend::new();
    let manager = TerminalManager::with_backend_and_interval(
        backend.clone(),
        pm.clone(),
        Duration::from_secs(60),
    );
    let events = RecordingEventSink::new();
    manager.set_event_sink(events.clone()).expect("sink");

    let cwd = work_dir.to_string_lossy().to_string();
    let s_fg = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-fg".to_string(),
                cwd: cwd.clone(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            RecordingDataSink::new(),
        )
        .expect("spawn fg");
    let h_fg = backend.last_handle();
    h_fg.set_foreground_child(true);
    manager
        .set_visibility(TerminalVisibilityArgs {
            session_id: s_fg.session_id.clone(),
            visible: false,
        })
        .expect("hide fg");

    let s_idle = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-idle".to_string(),
                cwd,
                cols: 80,
                rows: 24,
                shell: None,
            },
            RecordingDataSink::new(),
        )
        .expect("spawn idle");
    manager
        .set_visibility(TerminalVisibilityArgs {
            session_id: s_idle.session_id.clone(),
            visible: false,
        })
        .expect("hide idle");

    let future_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
        + 31 * 60_000;

    // First sweep updates foreground flag for s_fg and recycles s_idle; second sweep keeps s_fg exempt.
    manager.sweep_idle_sessions(future_now);
    manager.sweep_idle_sessions(future_now);

    let remaining = manager.list_sessions().expect("list");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].session_id, s_fg.session_id);

    let exits = events.exits();
    assert!(
        exits
            .iter()
            .any(|e| e.session_id == s_idle.session_id
                && e.reason == TerminalExitReason::IdleRecycled)
    );

    // Also verify idle_recycle_minutes = 0 exempts hidden sessions.
    pm.settings_update(SettingsPatch {
        panels: Some(PanelsSettingsPatch {
            terminal: Some(TerminalPanelSettingsPatch {
                idle_recycle_minutes: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    })
    .expect("disable idle recycle");
    h_fg.set_foreground_child(false);
    manager.sweep_idle_sessions(future_now + 60 * 60_000);
    assert_eq!(manager.list_sessions().expect("list").len(), 1);

    manager.shutdown_all();
    assert!(events
        .exits()
        .iter()
        .any(|e| e.session_id == s_fg.session_id && e.reason == TerminalExitReason::AppExit));

    let _ = std::fs::remove_dir_all(&app_dir);
    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn backpressure_accumulates_across_one_second_and_resets_on_user_input_or_quiet_gap() {
    let app_dir = temp_dir("backpressure-sustained");
    let work_dir = temp_dir("backpressure-sustained-cwd");
    let pm = Arc::new(PersistenceManager::open(app_dir.clone()).expect("open pm"));
    let backend = MockPtyBackend::new();
    let manager =
        TerminalManager::with_backend_and_interval(backend.clone(), pm, Duration::from_secs(60));
    let sink = RecordingDataSink::new();

    let info = manager
        .spawn_session(
            TerminalSpawnArgs {
                workspace_key: "ws-bp-sustained".to_string(),
                cwd: work_dir.to_string_lossy().to_string(),
                cols: 80,
                rows: 24,
                shell: None,
            },
            sink,
        )
        .expect("spawn");
    let handle = backend.last_handle();

    let six_mb = vec![b'A'; 6 * 1024 * 1024];
    for chunk in six_mb.chunks(256 * 1024) {
        handle.push_output(chunk);
    }
    thread::sleep(Duration::from_millis(80));

    // User input resets continuous counter when not paused.
    manager
        .write_session(TerminalWriteArgs {
            session_id: info.session_id.clone(),
            data_base64: "bHMK".to_string(),
        })
        .expect("write input");

    for chunk in six_mb.chunks(256 * 1024) {
        handle.push_output(chunk);
    }
    thread::sleep(Duration::from_millis(100));
    assert!(
        manager
            .list_sessions()
            .expect("list")
            .iter()
            .any(|s| s.session_id == info.session_id && !s.output_paused),
        "user input should have reset continuous output counter"
    );

    // Wait 1.1s (< 2s quiet threshold) then push 5MB -> total 11MB >= 10MB, must pause.
    thread::sleep(Duration::from_millis(1_100));
    let five_mb = vec![b'B'; 5 * 1024 * 1024];
    for chunk in five_mb.chunks(256 * 1024) {
        handle.push_output(chunk);
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut paused = false;
    while Instant::now() < deadline {
        if manager
            .list_sessions()
            .expect("list")
            .iter()
            .any(|s| s.session_id == info.session_id && s.output_paused)
        {
            paused = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        paused,
        "sustained output across 1s without >=2s quiet gap must trigger backpressure"
    );

    // Resume visibility, push 6MB, wait >=2.1s quiet gap, push another 6MB -> must NOT pause.
    manager
        .set_visibility(TerminalVisibilityArgs {
            session_id: info.session_id.clone(),
            visible: true,
        })
        .expect("resume");

    for chunk in six_mb.chunks(256 * 1024) {
        handle.push_output(chunk);
    }
    thread::sleep(Duration::from_millis(2_150));
    for chunk in six_mb.chunks(256 * 1024) {
        handle.push_output(chunk);
    }
    thread::sleep(Duration::from_millis(120));

    assert!(
        manager
            .list_sessions()
            .expect("list")
            .iter()
            .any(|s| s.session_id == info.session_id && !s.output_paused),
        "quiet gap >= 2s should reset continuous output counter"
    );

    manager.shutdown_all();
    let _ = std::fs::remove_dir_all(&app_dir);
    let _ = std::fs::remove_dir_all(&work_dir);
}
