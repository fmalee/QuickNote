use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter};

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::terminal::{
    decode_base64, encode_base64, should_idle_recycle, TerminalData, TerminalExitPayload,
    TerminalExitReason, TerminalKillArgs, TerminalLifecycleState, TerminalResizeArgs,
    TerminalRingBuffer, TerminalSessionInfo, TerminalSpawnArgs, TerminalVisibilityArgs,
    TerminalWriteArgs, TERMINAL_BACKPRESSURE_LIMIT_BYTES, TERMINAL_FLUSH_BATCH_BYTES,
    TERMINAL_FLUSH_INTERVAL_MS, TERMINAL_IDLE_SWEEP_INTERVAL_SECS,
};
use crate::infra::pty::{
    resolve_terminal_cwd, resolve_terminal_shell, PortablePtyBackend, PtyBackend, PtySessionHandle,
    PtySpawnConfig,
};

/// Backpressure measurement window in milliseconds (1 second).
const BACKPRESSURE_WINDOW_MS: i64 = 1_000;

/// Sink for streaming PTY output chunks (`Channel<TerminalData>` in production).
pub trait TerminalDataSink: Send + Sync {
    fn send(&self, data: TerminalData) -> Result<(), AppError>;
}

/// Production `tauri::ipc::Channel` wrapper.
pub struct TauriTerminalDataSink {
    channel: Channel<TerminalData>,
}

impl TauriTerminalDataSink {
    pub fn new(channel: Channel<TerminalData>) -> Self {
        Self { channel }
    }
}

impl TerminalDataSink for TauriTerminalDataSink {
    fn send(&self, data: TerminalData) -> Result<(), AppError> {
        self.channel.send(data).map_err(|err| {
            AppError::with_detail(
                ErrorCode::InternalError,
                "Failed to send terminal data over IPC channel",
                err.to_string(),
            )
        })
    }
}

/// Sink for terminal lifecycle and exit events (`AppHandle::emit` in production).
pub trait TerminalEventSink: Send + Sync {
    fn emit_session_state(&self, info: &TerminalSessionInfo);
    fn emit_exit(&self, payload: &TerminalExitPayload);
}

/// Production `AppHandle` event emitter.
pub struct TauriTerminalEventSink {
    app: AppHandle,
}

impl TauriTerminalEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl TerminalEventSink for TauriTerminalEventSink {
    fn emit_session_state(&self, info: &TerminalSessionInfo) {
        if let Err(err) = self.app.emit("terminal:session-state", info) {
            eprintln!(
                "[terminal] emit terminal:session-state failed for session={}: {err}",
                info.session_id
            );
        }
    }

    fn emit_exit(&self, payload: &TerminalExitPayload) {
        if let Err(err) = self.app.emit("terminal:exit", payload) {
            eprintln!(
                "[terminal] emit terminal:exit failed for session={}: {err}",
                payload.session_id
            );
        }
    }
}

struct SessionMutableState {
    state: TerminalLifecycleState,
    has_active_foreground_child: bool,
    output_paused: bool,
    last_io_at_ms: i64,
    window_started_at_ms: i64,
    unconsumed_bytes_in_window: usize,
    ring_buffer: TerminalRingBuffer,
    exited: bool,
}

struct TerminalSessionEntry {
    session_id: String,
    workspace_key: String,
    cwd: String,
    pty: Arc<dyn PtySessionHandle>,
    data_sink: Arc<dyn TerminalDataSink>,
    mutable: Mutex<SessionMutableState>,
}

impl TerminalSessionEntry {
    fn snapshot(&self, guard: &SessionMutableState) -> TerminalSessionInfo {
        TerminalSessionInfo {
            session_id: self.session_id.clone(),
            workspace_key: self.workspace_key.clone(),
            cwd: self.cwd.clone(),
            has_active_foreground_child: guard.has_active_foreground_child,
            output_paused: guard.output_paused,
            state: guard.state,
        }
    }
}

/// Manages embedded PTY sessions, fold-keepalive ring buffers, backpressure, and idle recycling (`C8.7`, `D-0005`).
pub struct TerminalManager {
    backend: Arc<dyn PtyBackend>,
    pm: Arc<PersistenceManager>,
    sessions: Arc<Mutex<HashMap<String, Arc<TerminalSessionEntry>>>>,
    event_sink: Arc<Mutex<Option<Arc<dyn TerminalEventSink>>>>,
    seq: AtomicU64,
    shutting_down: AtomicBool,
    stop_signal: Arc<(Mutex<bool>, Condvar)>,
    sweep_thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.stop_signal;
        if let Ok(mut stopped) = lock.lock() {
            *stopped = true;
            cvar.notify_all();
        }
        if let Ok(mut handle_guard) = self.sweep_thread.lock() {
            if let Some(handle) = handle_guard.take() {
                let _ = handle.join();
            }
        }
    }
}

impl TerminalManager {
    /// Creates a production `TerminalManager` backed by `PortablePtyBackend` and a 60s idle sweep.
    pub fn new(pm: Arc<PersistenceManager>) -> Arc<Self> {
        Self::with_backend_and_interval(
            Arc::new(PortablePtyBackend::new()),
            pm,
            Duration::from_secs(TERMINAL_IDLE_SWEEP_INTERVAL_SECS),
        )
    }

    /// Creates a `TerminalManager` with an injected PTY backend and configurable sweep interval.
    pub fn with_backend_and_interval(
        backend: Arc<dyn PtyBackend>,
        pm: Arc<PersistenceManager>,
        sweep_interval: Duration,
    ) -> Arc<Self> {
        let manager = Arc::new(Self {
            backend,
            pm,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            event_sink: Arc::new(Mutex::new(None)),
            seq: AtomicU64::new(1),
            shutting_down: AtomicBool::new(false),
            stop_signal: Arc::new((Mutex::new(false), Condvar::new())),
            sweep_thread: Mutex::new(None),
        });

        let weak = Arc::downgrade(&manager);
        let stop_signal = Arc::clone(&manager.stop_signal);
        let tick_step = sweep_interval
            .min(Duration::from_secs(1))
            .max(Duration::from_millis(50));

        let handle = thread::spawn(move || {
            let mut elapsed_since_sweep = Duration::ZERO;
            loop {
                let (lock, cvar) = &*stop_signal;
                let guard = match lock.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if *guard {
                    break;
                }
                let (guard, _) = match cvar.wait_timeout(guard, tick_step) {
                    Ok(res) => res,
                    Err(_) => break,
                };
                if *guard {
                    break;
                }
                drop(guard);

                let Some(mgr) = weak.upgrade() else {
                    break;
                };
                mgr.refresh_foreground_children();
                elapsed_since_sweep += tick_step;
                if elapsed_since_sweep >= sweep_interval {
                    elapsed_since_sweep = Duration::ZERO;
                    mgr.sweep_idle_sessions(now_ms());
                }
            }
        });

        if let Ok(mut guard) = manager.sweep_thread.lock() {
            *guard = Some(handle);
        }

        manager
    }

    /// Attaches an event sink (typically `TauriTerminalEventSink` in `lib.rs` or a mock in tests).
    pub fn set_event_sink(&self, sink: Arc<dyn TerminalEventSink>) -> Result<(), AppError> {
        let mut guard = self.event_sink.lock().map_err(|_| {
            AppError::new(
                ErrorCode::InternalError,
                "Terminal event sink mutex poisoned",
            )
        })?;
        *guard = Some(sink);
        Ok(())
    }

    fn current_event_sink(&self) -> Option<Arc<dyn TerminalEventSink>> {
        self.event_sink
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(Arc::clone))
    }

    fn emit_session_state(&self, info: &TerminalSessionInfo) {
        if let Some(sink) = self.current_event_sink() {
            sink.emit_session_state(info);
        }
    }

    fn emit_exit(&self, payload: &TerminalExitPayload) {
        if let Some(sink) = self.current_event_sink() {
            sink.emit_exit(payload);
        }
    }

    fn next_session_id(&self) -> String {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        format!("term-{}-{seq}", now_ms())
    }

    /// Spawns a new workspace-scoped PTY session (`C4.2`, `C8.7`).
    pub fn spawn_session(
        self: &Arc<Self>,
        args: TerminalSpawnArgs,
        data_sink: Arc<dyn TerminalDataSink>,
    ) -> Result<TerminalSessionInfo, AppError> {
        let workspace_key = args.workspace_key.trim().to_string();
        if workspace_key.is_empty() {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "workspaceKey cannot be empty",
            ));
        }
        if args.cols == 0 || args.rows == 0 {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Terminal cols and rows must be greater than zero",
            ));
        }

        let snapshot = self.pm.settings_get()?;
        let term_settings = &snapshot.settings.panels.terminal;
        let max_concurrent = (term_settings.max_concurrent_sessions.max(1)) as usize;

        {
            let sessions = self.sessions.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal sessions mutex poisoned")
            })?;
            if sessions.values().any(|s| s.workspace_key == workspace_key) {
                return Err(AppError::new(
                    ErrorCode::TerminalLimitReached,
                    "Workspace already has an active terminal session",
                ));
            }
            if sessions.len() >= max_concurrent {
                return Err(AppError::new(
                    ErrorCode::TerminalLimitReached,
                    "Maximum concurrent terminal sessions reached",
                ));
            }
        }

        let cwd_path = resolve_terminal_cwd(&args.cwd)?;
        let shell = resolve_terminal_shell(args.shell.as_deref(), &term_settings.shell);
        let pty = self.backend.spawn(&PtySpawnConfig {
            cwd: cwd_path.clone(),
            cols: args.cols,
            rows: args.rows,
            shell,
        })?;

        let reader = pty.take_reader()?;
        let session_id = self.next_session_id();
        let now = now_ms();
        let has_fg = pty.has_active_foreground_child();

        let entry = Arc::new(TerminalSessionEntry {
            session_id: session_id.clone(),
            workspace_key: workspace_key.clone(),
            cwd: cwd_path.to_string_lossy().to_string(),
            pty,
            data_sink,
            mutable: Mutex::new(SessionMutableState {
                state: TerminalLifecycleState::Active,
                has_active_foreground_child: has_fg,
                output_paused: false,
                last_io_at_ms: now,
                window_started_at_ms: now,
                unconsumed_bytes_in_window: 0,
                ring_buffer: TerminalRingBuffer::new(),
                exited: false,
            }),
        });

        let info = {
            let mut sessions = self.sessions.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal sessions mutex poisoned")
            })?;
            // Re-check limits under lock in case of concurrent spawns.
            if sessions.values().any(|s| s.workspace_key == workspace_key)
                || sessions.len() >= max_concurrent
            {
                drop(sessions);
                let _ = entry.pty.terminate_process_group();
                return Err(AppError::new(
                    ErrorCode::TerminalLimitReached,
                    "Terminal session limit reached",
                ));
            }
            sessions.insert(session_id.clone(), Arc::clone(&entry));
            let guard = entry.mutable.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal state mutex poisoned")
            })?;
            entry.snapshot(&guard)
        };

        self.emit_session_state(&info);
        self.spawn_io_threads(Arc::clone(&entry), reader);
        Ok(info)
    }

    fn spawn_io_threads(
        self: &Arc<Self>,
        entry: Arc<TerminalSessionEntry>,
        mut reader: Box<dyn Read + Send>,
    ) {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let sid_for_reader = entry.session_id.clone();

        thread::spawn(move || {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "[terminal] PTY read ended for session={sid_for_reader}: kind={:?}",
                            err.kind()
                        );
                        break;
                    }
                }
            }
        });

        let manager = Arc::clone(self);
        thread::spawn(move || {
            let mut pending: Vec<u8> = Vec::new();
            let flush_interval = Duration::from_millis(TERMINAL_FLUSH_INTERVAL_MS);

            loop {
                match rx.recv_timeout(flush_interval) {
                    Ok(chunk) => {
                        pending.extend_from_slice(&chunk);
                        if pending.len() >= TERMINAL_FLUSH_BATCH_BYTES {
                            manager.flush_output(&entry, &pending);
                            pending.clear();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if !pending.is_empty() {
                            manager.flush_output(&entry, &pending);
                            pending.clear();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        if !pending.is_empty() {
                            manager.flush_output(&entry, &pending);
                            pending.clear();
                        }
                        break;
                    }
                }
            }

            manager.handle_natural_process_exit(&entry);
        });
    }

    fn flush_output(&self, entry: &TerminalSessionEntry, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let now = now_ms();
        let mut should_emit_state: Option<TerminalSessionInfo> = None;
        let mut to_send: Option<Vec<u8>> = None;

        {
            let Ok(mut guard) = entry.mutable.lock() else {
                return;
            };
            if guard.exited {
                return;
            }
            guard.last_io_at_ms = now;

            match guard.state {
                TerminalLifecycleState::HiddenAlive => {
                    guard.ring_buffer.push(bytes);
                }
                TerminalLifecycleState::Active => {
                    if guard.output_paused {
                        // Drop overflow bytes while paused so PTY fd never deadlocks.
                        return;
                    }
                    if now.saturating_sub(guard.window_started_at_ms) >= BACKPRESSURE_WINDOW_MS {
                        guard.window_started_at_ms = now;
                        guard.unconsumed_bytes_in_window = 0;
                    }
                    guard.unconsumed_bytes_in_window =
                        guard.unconsumed_bytes_in_window.saturating_add(bytes.len());

                    if guard.unconsumed_bytes_in_window >= TERMINAL_BACKPRESSURE_LIMIT_BYTES {
                        guard.output_paused = true;
                        should_emit_state = Some(entry.snapshot(&guard));
                    } else {
                        to_send = Some(bytes.to_vec());
                    }
                }
                TerminalLifecycleState::Recycled => return,
            }
        }

        if let Some(info) = should_emit_state {
            self.emit_session_state(&info);
        }

        if let Some(payload_bytes) = to_send {
            self.send_chunked(entry, &payload_bytes);
        }
    }

    fn send_chunked(&self, entry: &TerminalSessionEntry, bytes: &[u8]) {
        for slice in bytes.chunks(TERMINAL_FLUSH_BATCH_BYTES) {
            let encoded = encode_base64(slice);
            if let Err(err) = entry.data_sink.send(TerminalData {
                session_id: entry.session_id.clone(),
                data_base64: encoded,
            }) {
                eprintln!(
                    "[terminal] data channel send failed for session={}: code={:?}",
                    entry.session_id, err.code
                );
                break;
            }
        }
    }

    fn handle_natural_process_exit(&self, entry: &TerminalSessionEntry) {
        if self.shutting_down.load(Ordering::Relaxed) {
            return;
        }

        let mut removed = false;
        if let Ok(mut sessions) = self.sessions.lock() {
            if sessions.remove(&entry.session_id).is_some() {
                removed = true;
            }
        }

        let state_snapshot = {
            let Ok(mut guard) = entry.mutable.lock() else {
                return;
            };
            if guard.exited {
                return;
            }
            guard.exited = true;
            guard.state = TerminalLifecycleState::Recycled;
            guard.has_active_foreground_child = false;
            entry.snapshot(&guard)
        };

        if !removed {
            return;
        }

        let exit_code = entry.pty.try_wait().ok().flatten();
        self.emit_session_state(&state_snapshot);
        self.emit_exit(&TerminalExitPayload {
            session_id: entry.session_id.clone(),
            exit_code,
            reason: TerminalExitReason::ProcessExit,
        });
    }

    /// Writes base64-decoded input bytes to the PTY session (`C4.2`).
    pub fn write_session(&self, args: TerminalWriteArgs) -> Result<(), AppError> {
        let decoded = decode_base64(&args.data_base64)?;
        let entry = self.get_session(&args.session_id)?;
        {
            let mut guard = entry.mutable.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal state mutex poisoned")
            })?;
            if guard.exited {
                return Err(AppError::new(
                    ErrorCode::NotFound,
                    "Terminal session has already exited",
                ));
            }
            guard.last_io_at_ms = now_ms();
        }
        entry.pty.write_all(&decoded)?;
        self.refresh_session_foreground(&entry);
        Ok(())
    }

    /// Resizes the PTY session dimensions (`C4.2`).
    pub fn resize_session(&self, args: TerminalResizeArgs) -> Result<(), AppError> {
        if args.cols == 0 || args.rows == 0 {
            return Err(AppError::new(
                ErrorCode::ValidationError,
                "Terminal cols and rows must be greater than zero",
            ));
        }
        let entry = self.get_session(&args.session_id)?;
        entry.pty.resize(args.cols, args.rows)
    }

    /// Toggles fold-keepalive visibility and flushes ring buffer / resets backpressure (`C8.7`).
    pub fn set_visibility(&self, args: TerminalVisibilityArgs) -> Result<(), AppError> {
        let entry = self.get_session(&args.session_id)?;
        let now = now_ms();

        let (snapshot, buffered) = {
            let mut guard = entry.mutable.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal state mutex poisoned")
            })?;
            if guard.exited {
                return Err(AppError::new(
                    ErrorCode::NotFound,
                    "Terminal session has already exited",
                ));
            }
            if args.visible {
                guard.state = TerminalLifecycleState::Active;
                guard.output_paused = false;
                guard.unconsumed_bytes_in_window = 0;
                guard.window_started_at_ms = now;
                let drained = guard.ring_buffer.drain_all();
                (entry.snapshot(&guard), Some(drained))
            } else {
                guard.state = TerminalLifecycleState::HiddenAlive;
                (entry.snapshot(&guard), None)
            }
        };

        self.emit_session_state(&snapshot);
        if let Some(bytes) = buffered {
            if !bytes.is_empty() {
                self.send_chunked(&entry, &bytes);
            }
        }
        Ok(())
    }

    /// Manually kills a terminal session and reaps its process group (`C8.7`).
    pub fn kill_session(&self, args: TerminalKillArgs) -> Result<(), AppError> {
        let entry = {
            let mut sessions = self.sessions.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal sessions mutex poisoned")
            })?;
            sessions
                .remove(&args.session_id)
                .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Terminal session not found"))?
        };

        let snapshot = {
            let mut guard = entry.mutable.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal state mutex poisoned")
            })?;
            guard.exited = true;
            guard.state = TerminalLifecycleState::Recycled;
            guard.has_active_foreground_child = false;
            entry.snapshot(&guard)
        };

        let exit_code = entry.pty.terminate_process_group()?;
        self.emit_session_state(&snapshot);
        self.emit_exit(&TerminalExitPayload {
            session_id: entry.session_id.clone(),
            exit_code,
            reason: TerminalExitReason::ManualClose,
        });
        Ok(())
    }

    /// Lists all currently alive (active or hidden) terminal sessions (`C4.2`).
    pub fn list_sessions(&self) -> Result<Vec<TerminalSessionInfo>, AppError> {
        let entries: Vec<Arc<TerminalSessionEntry>> = {
            let sessions = self.sessions.lock().map_err(|_| {
                AppError::new(ErrorCode::InternalError, "Terminal sessions mutex poisoned")
            })?;
            sessions.values().map(Arc::clone).collect()
        };

        let mut list = Vec::with_capacity(entries.len());
        for entry in entries {
            if let Ok(guard) = entry.mutable.lock() {
                if !guard.exited {
                    list.push(entry.snapshot(&guard));
                }
            }
        }
        Ok(list)
    }

    /// Sweeps hidden sessions exceeding `idle_recycle_minutes` without foreground child (`C8.7`).
    pub fn sweep_idle_sessions(&self, now: i64) {
        let idle_recycle_minutes = self
            .pm
            .settings_get()
            .map(|s| s.settings.panels.terminal.idle_recycle_minutes)
            .unwrap_or(30);

        let entries: Vec<Arc<TerminalSessionEntry>> = match self.sessions.lock() {
            Ok(sessions) => sessions.values().map(Arc::clone).collect(),
            Err(_) => return,
        };

        for entry in entries {
            let has_fg = entry.pty.has_active_foreground_child();
            let should_recycle = {
                let Ok(mut guard) = entry.mutable.lock() else {
                    continue;
                };
                if guard.exited {
                    continue;
                }
                if guard.has_active_foreground_child != has_fg {
                    guard.has_active_foreground_child = has_fg;
                    let info = entry.snapshot(&guard);
                    drop(guard);
                    self.emit_session_state(&info);
                    continue;
                }
                should_idle_recycle(
                    guard.state,
                    idle_recycle_minutes,
                    guard.has_active_foreground_child,
                    guard.last_io_at_ms,
                    now,
                )
            };

            if should_recycle {
                let removed = match self.sessions.lock() {
                    Ok(mut sessions) => sessions.remove(&entry.session_id).is_some(),
                    Err(_) => false,
                };
                if !removed {
                    continue;
                }

                let snapshot = {
                    let Ok(mut guard) = entry.mutable.lock() else {
                        continue;
                    };
                    if guard.exited {
                        continue;
                    }
                    guard.exited = true;
                    guard.state = TerminalLifecycleState::Recycled;
                    guard.has_active_foreground_child = false;
                    entry.snapshot(&guard)
                };

                let exit_code = match entry.pty.terminate_process_group() {
                    Ok(code) => code,
                    Err(err) => {
                        eprintln!(
                            "[terminal] idle recycle terminate failed for session={}: code={:?}",
                            entry.session_id, err.code
                        );
                        None
                    }
                };
                self.emit_session_state(&snapshot);
                self.emit_exit(&TerminalExitPayload {
                    session_id: entry.session_id.clone(),
                    exit_code,
                    reason: TerminalExitReason::IdleRecycled,
                });
            }
        }
    }

    /// Terminates all active/hidden sessions cleanly on application exit (`C8.7`).
    pub fn shutdown_all(&self) {
        self.shutting_down.store(true, Ordering::Relaxed);
        let (lock, cvar) = &*self.stop_signal;
        if let Ok(mut stopped) = lock.lock() {
            *stopped = true;
            cvar.notify_all();
        }

        let entries: Vec<Arc<TerminalSessionEntry>> = match self.sessions.lock() {
            Ok(mut sessions) => sessions.drain().map(|(_, v)| v).collect(),
            Err(_) => return,
        };

        for entry in entries {
            let snapshot = {
                let Ok(mut guard) = entry.mutable.lock() else {
                    continue;
                };
                if guard.exited {
                    continue;
                }
                guard.exited = true;
                guard.state = TerminalLifecycleState::Recycled;
                guard.has_active_foreground_child = false;
                entry.snapshot(&guard)
            };

            let exit_code = match entry.pty.terminate_process_group() {
                Ok(code) => code,
                Err(err) => {
                    eprintln!(
                        "[terminal] app exit terminate failed for session={}: code={:?}",
                        entry.session_id, err.code
                    );
                    None
                }
            };
            self.emit_session_state(&snapshot);
            self.emit_exit(&TerminalExitPayload {
                session_id: entry.session_id.clone(),
                exit_code,
                reason: TerminalExitReason::AppExit,
            });
        }
    }

    fn refresh_foreground_children(&self) {
        let entries: Vec<Arc<TerminalSessionEntry>> = match self.sessions.lock() {
            Ok(sessions) => sessions.values().map(Arc::clone).collect(),
            Err(_) => return,
        };
        for entry in entries {
            self.refresh_session_foreground(&entry);
        }
    }

    fn refresh_session_foreground(&self, entry: &TerminalSessionEntry) {
        let has_fg = entry.pty.has_active_foreground_child();
        let changed_info = {
            let Ok(mut guard) = entry.mutable.lock() else {
                return;
            };
            if guard.exited || guard.has_active_foreground_child == has_fg {
                None
            } else {
                guard.has_active_foreground_child = has_fg;
                Some(entry.snapshot(&guard))
            }
        };
        if let Some(info) = changed_info {
            self.emit_session_state(&info);
        }
    }

    fn get_session(&self, session_id: &str) -> Result<Arc<TerminalSessionEntry>, AppError> {
        let sessions = self.sessions.lock().map_err(|_| {
            AppError::new(ErrorCode::InternalError, "Terminal sessions mutex poisoned")
        })?;
        sessions
            .get(session_id)
            .map(Arc::clone)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Terminal session not found"))
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
