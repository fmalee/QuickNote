use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter};

use crate::application::persistence::PersistenceManager;
use crate::domain::error::{AppError, ErrorCode};
use crate::domain::note::parse_note;
use crate::domain::terminal::{
    decode_base64, encode_base64, should_idle_recycle, TerminalData, TerminalExitPayload,
    TerminalExitReason, TerminalKillArgs, TerminalLifecycleState, TerminalResizeArgs,
    TerminalRingBuffer, TerminalSessionInfo, TerminalSpawnArgs, TerminalVisibilityArgs,
    TerminalWriteArgs, TERMINAL_BACKPRESSURE_LIMIT_BYTES, TERMINAL_FLUSH_BATCH_BYTES,
    TERMINAL_FLUSH_INTERVAL_MS, TERMINAL_IDLE_SWEEP_INTERVAL_SECS,
};
use crate::domain::workspace_context::{
    resolve_effective_workspace, DirectoryWorkspaceConfig, DIRECTORY_CONFIG_FILENAME,
};
use crate::infra::pty::{
    resolve_terminal_cwd as resolve_pty_cwd, resolve_terminal_shell, PortablePtyBackend,
    PtyBackend, PtySessionHandle, PtySpawnConfig,
};
use crate::infra::sqlite::list_ancestor_directory_workspace_configs;

/// Quiet gap threshold in milliseconds after which continuous output counter resets (2 seconds).
const BACKPRESSURE_QUIET_RESET_MS: i64 = 2_000;

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
    last_output_at_ms: i64,
    continuous_output_bytes: usize,
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

    /// Resolves the effective terminal working directory for an optional active document path.
    ///
    /// Resolution rules:
    /// - Existing directory path: validated and returned directly.
    /// - External file (outside `library_root`): returns the file's parent directory.
    /// - Library document (inside `library_root`): resolves 3-tier workspace cascade
    ///   (`Frontmatter > nearest ancestor .quicknote.yaml > document directory`),
    ///   preferring `default_terminal_cwd`, then `project_root`, falling back to the
    ///   document's parent directory.
    /// - Empty / `None`: falls back to `library_root` when configured.
    pub fn resolve_terminal_cwd(&self, active_doc_path: Option<&str>) -> Result<PathBuf, AppError> {
        let library_root = self
            .pm
            .settings_get()
            .ok()
            .and_then(|snapshot| snapshot.settings.library_root)
            .map(|root| root.trim().to_string())
            .filter(|root| !root.is_empty())
            .map(|root| expand_tilde_path(&root));

        let Some(raw_input) = active_doc_path.map(str::trim).filter(|s| !s.is_empty()) else {
            if let Some(root) = library_root {
                return resolve_pty_cwd(&root.to_string_lossy());
            }
            return Err(AppError::new(
                ErrorCode::WorkspaceDirMissing,
                "Terminal working directory cannot be empty",
            ));
        };

        let candidate = expand_tilde_path(raw_input);
        if candidate.is_dir() {
            return resolve_pty_cwd(&candidate.to_string_lossy());
        }
        if !candidate.is_file() {
            return resolve_pty_cwd(raw_input);
        }

        let parent_dir = candidate.parent().ok_or_else(|| {
            AppError::new(
                ErrorCode::WorkspaceDirMissing,
                "Active document has no parent directory",
            )
        })?;

        if let Some(ref root) = library_root {
            if let Some(vault_rel_dir) = vault_rel_dir_for_abs_path(&candidate, root) {
                let doc_layer = read_doc_workspace_layer(&candidate);
                let ancestor_layers =
                    load_ancestor_workspace_layers(&self.pm, &candidate, root, &vault_rel_dir);
                let effective =
                    resolve_effective_workspace(doc_layer.as_ref(), &ancestor_layers, None);

                for preferred in [
                    effective.default_terminal_cwd.as_deref(),
                    effective.project_root.as_deref(),
                ] {
                    if let Some(raw_dir) = preferred.map(str::trim).filter(|s| !s.is_empty()) {
                        let resolved_dir = resolve_configured_dir(raw_dir, root);
                        if let Ok(valid) = resolve_pty_cwd(&resolved_dir.to_string_lossy()) {
                            return Ok(valid);
                        }
                    }
                }

                return resolve_pty_cwd(&parent_dir.to_string_lossy());
            }
        }

        // External file: always default terminal cwd to the file's parent directory.
        resolve_pty_cwd(&parent_dir.to_string_lossy())
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

        let cwd_path = self.resolve_terminal_cwd(Some(&args.cwd))?;
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
                last_output_at_ms: now,
                continuous_output_bytes: 0,
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
                    let prev_output_at_ms = guard.last_output_at_ms;
                    guard.last_output_at_ms = now;
                    if now.saturating_sub(prev_output_at_ms) >= BACKPRESSURE_QUIET_RESET_MS {
                        guard.continuous_output_bytes = 0;
                    }
                    guard.continuous_output_bytes =
                        guard.continuous_output_bytes.saturating_add(bytes.len());

                    if guard.continuous_output_bytes >= TERMINAL_BACKPRESSURE_LIMIT_BYTES {
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
            if !guard.output_paused {
                guard.continuous_output_bytes = 0;
            }
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
                guard.continuous_output_bytes = 0;
                guard.last_output_at_ms = now;
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

fn expand_tilde_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn resolve_configured_dir(raw: &str, library_root: &Path) -> PathBuf {
    let expanded = expand_tilde_path(raw);
    if expanded.is_absolute() {
        expanded
    } else {
        library_root.join(expanded)
    }
}

fn normalize_vault_rel_dir(raw: &str) -> String {
    raw.replace('\\', "/")
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn vault_rel_dir_for_abs_path(abs_path: &Path, library_root: &Path) -> Option<String> {
    let rel = if let Ok(r) = abs_path.strip_prefix(library_root) {
        r.to_path_buf()
    } else {
        let canonical_abs = fs::canonicalize(abs_path).ok()?;
        let canonical_root = fs::canonicalize(library_root).ok()?;
        canonical_abs
            .strip_prefix(&canonical_root)
            .ok()?
            .to_path_buf()
    };

    let rel_dir = if abs_path.is_dir() {
        rel.as_path()
    } else {
        rel.parent().unwrap_or_else(|| Path::new(""))
    };
    Some(normalize_vault_rel_dir(&rel_dir.to_string_lossy()))
}

fn read_doc_workspace_layer(path: &Path) -> Option<DirectoryWorkspaceConfig> {
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
    let parsed = parse_note(&raw, stem);
    Some(DirectoryWorkspaceConfig::from_note_meta(&parsed.meta))
}

fn load_ancestor_workspace_layers(
    pm: &PersistenceManager,
    abs_path: &Path,
    library_root: &Path,
    vault_rel_dir: &str,
) -> Vec<DirectoryWorkspaceConfig> {
    if let Ok(records) =
        pm.with_db(|conn| list_ancestor_directory_workspace_configs(conn, vault_rel_dir))
    {
        if !records.is_empty() {
            let mut layers = Vec::with_capacity(records.len());
            for rec in records {
                if rec.parse_error.is_some() {
                    continue;
                }
                if let Ok(cfg) = serde_json::from_str::<DirectoryWorkspaceConfig>(&rec.config_json)
                {
                    layers.push(cfg);
                }
            }
            if !layers.is_empty() {
                return layers;
            }
        }
    }

    // Read-only filesystem fallback when SQLite cache is not populated yet.
    let canonical_root =
        fs::canonicalize(library_root).unwrap_or_else(|_| library_root.to_path_buf());
    let start_dir = if abs_path.is_dir() {
        abs_path.to_path_buf()
    } else {
        abs_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| library_root.to_path_buf())
    };
    let mut cursor = fs::canonicalize(&start_dir).unwrap_or(start_dir);
    let mut layers = Vec::new();
    loop {
        let cfg_path = cursor.join(DIRECTORY_CONFIG_FILENAME);
        if cfg_path.is_file() {
            if let Ok(raw) = fs::read_to_string(&cfg_path) {
                if let Ok(cfg) = serde_yaml::from_str::<DirectoryWorkspaceConfig>(&raw) {
                    layers.push(cfg);
                }
            }
        }
        if cursor == canonical_root || !cursor.starts_with(&canonical_root) {
            break;
        }
        let Some(parent) = cursor.parent() else {
            break;
        };
        if parent == cursor {
            break;
        }
        cursor = parent.to_path_buf();
    }
    layers
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
