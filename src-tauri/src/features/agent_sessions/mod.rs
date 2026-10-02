//! Live agent sessions in pseudo-terminals (ConPTY on Windows).
//!
//! The engine runs whatever [`LaunchSpec`] it's given. Building the real `claude` spec needs
//! the roster and launch message (P6), the subscription preflight (P3) and the hook server
//! (P8); until those exist, `agent_start` is a stub and the engine is driven by tests.
//!
//! Live state goes through `core::sessions`, so other features can ask whether a project has
//! a running session. Raw terminal output is never written to disk (it can hold secrets):
//! it lives only in each session's in-memory ring buffer.

pub mod commands;
mod ring;

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::core::events::EventMeta;
use crate::core::process_tree::ProcessTree;
use crate::core::repos::RepoIndex;
use crate::core::sessions::{now_ms, SessionInfo, SessionRegistry, SessionStatus};
use crate::core::subscription::subscription_env;
use crate::core::{AppError, ErrorCode};
use ring::Ring;

/// Scrollback kept per agent for `agent_snapshot`.
pub const RING_BYTES: usize = 256 * 1024;
/// Output is coalesced into batches this long.
pub const BATCH: Duration = Duration::from_millis(16);
/// A batch is flushed early once it reaches this size.
const MAX_BATCH_BYTES: usize = 64 * 1024;
/// How long `agent_stop` waits after the interrupt before killing the tree.
pub const STOP_GRACE: Duration = Duration::from_secs(5);
/// How long to wait for the process to be gone after killing its tree.
const KILL_WAIT: Duration = Duration::from_secs(3);
const READ_CHUNK: usize = 8 * 1024;
/// Ctrl+C, written to the terminal's input.
const INTERRUPT: &[u8] = b"\x03";
/// "Where is the cursor?" (DSR). ConPTY sends it once at startup and blocks the program until
/// it's answered (portable-pty creates it with `PSEUDOCONSOLE_INHERIT_CURSOR`).
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
/// Our answer to that startup query: top-left.
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";

/// Values for the hook helper (P8) to reach Foundry.
#[derive(Debug, Clone)]
pub struct HookEnv {
    pub port: u16,
    pub token: String,
}

/// Exactly what to run for one agent session.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub agent_id: String,
    pub project_id: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The linked repo.
    pub cwd: PathBuf,
    pub hook: HookEnv,
    pub cols: u16,
    pub rows: u16,
}

/// The `claude` to launch, from the preflight: the executable and the `defaultModel`.
#[derive(Debug, Clone)]
pub struct ClaudeLaunch {
    pub program: PathBuf,
    pub model: Option<String>,
}

/// One agent's interactive Claude Code session: `claude [--model <m>] "<launch message>"`
/// in the project's repo.
#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub project_id: String,
    pub agent_id: String,
    /// The initial prompt (P6's `launch_message`).
    pub launch_message: String,
    pub hook: HookEnv,
    pub cols: u16,
    pub rows: u16,
}

/// What `launch` needs beyond the engine: where repos are (and which are archived), and the
/// preflight that finds `claude` and checks the subscription login.
struct Launcher {
    repos: Arc<RepoIndex>,
    preflight: Box<dyn Fn() -> Result<ClaudeLaunch, AppError> + Send + Sync>,
}

/// `agent:output`: a coalesced batch of raw terminal output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "agent:output")]
pub struct AgentOutput {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub agent_id: String,
    /// Base64 of the raw bytes (may split a UTF-8 sequence or an escape code).
    pub chunk: String,
}

impl AgentOutput {
    pub fn new(agent_id: &str, bytes: &[u8]) -> Self {
        Self {
            meta: EventMeta::new(),
            agent_id: agent_id.to_string(),
            chunk: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }
}

/// `agent:status`: a session changed state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "agent:status")]
pub struct AgentStatus {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub agent_id: String,
    pub status: SessionStatus,
    /// Set with `exited` when the process reported one.
    pub exit_code: Option<u32>,
    pub pid: Option<u32>,
    /// Epoch ms of this change.
    // u32 stands in for the exported type only (TS `number`), as with EventMeta.ts.
    #[specta(type = u32)]
    pub since: u64,
}

impl AgentStatus {
    fn new(
        agent_id: &str,
        status: SessionStatus,
        exit_code: Option<u32>,
        pid: Option<u32>,
    ) -> Self {
        Self {
            meta: EventMeta::new(),
            agent_id: agent_id.to_string(),
            status,
            exit_code,
            pid,
            since: now_ms(),
        }
    }
}

/// Where session events go: the app emits them to the webview, tests record them.
pub trait SessionEvents: Send + Sync + 'static {
    fn output(&self, event: AgentOutput);
    fn status(&self, event: AgentStatus);
}

/// What every session adds on top of [`subscription_env`]: the four `FOUNDRY_*` variables
/// and `TERM`.
fn session_vars(spec: &LaunchSpec) -> [(&'static str, String); 5] {
    [
        ("FOUNDRY_AGENT_ID", spec.agent_id.clone()),
        ("FOUNDRY_PROJECT_ID", spec.project_id.clone()),
        ("FOUNDRY_HOOK_PORT", spec.hook.port.to_string()),
        ("FOUNDRY_HOOK_TOKEN", spec.hook.token.clone()),
        ("TERM", "xterm-256color".to_string()),
    ]
}

/// `subscription_env()` plus the session's variables. An inherited variable of the same name
/// (say, a `FOUNDRY_HOOK_TOKEN` from a parent Foundry) is replaced, not duplicated.
fn child_env(spec: &LaunchSpec) -> Vec<(OsString, OsString)> {
    let ours = session_vars(spec);
    let mut env: Vec<(OsString, OsString)> = subscription_env()
        .into_iter()
        .filter(|(key, _)| !ours.iter().any(|(name, _)| key.eq_ignore_ascii_case(name)))
        .collect();
    env.extend(
        ours.into_iter()
            .map(|(key, value)| (key.into(), value.into())),
    );
    env
}

/// Agent sessions run `claude` only as a real executable, never through a `.cmd`/`.bat` shim
/// (npm's `claude.cmd`). Windows starts a shim via `cmd.exe`, which re-parses the command
/// line, and the launch message is on that line; no character filter can be shown to be
/// complete for cmd.exe's parsing, so shims aren't used at all.
fn require_native(program: &Path) -> Result<(), AppError> {
    let is_shim = program
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"));
    if is_shim {
        return Err(AppError::new(
            ErrorCode::ClaudeNeedsNative,
            format!(
                "Agent sessions need the native Claude Code (claude.exe), but Foundry found the \
                 npm shim at {}. To fix it, run `claude install` in a terminal to install the \
                 native build, then set Claude path in Foundry's settings to the claude.exe it \
                 installs (by default %USERPROFILE%\\.local\\bin\\claude.exe).",
                program.display()
            ),
        ));
    }
    Ok(())
}

fn pty_error(what: &str, err: impl std::fmt::Display) -> AppError {
    AppError::internal(format!("{what}: {err}"))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Every guarded value stays valid if a holder panicked mid-way (plain I/O handles).
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Set once the process has exited.
#[derive(Default)]
struct Exit {
    code: Mutex<Option<Option<u32>>>,
    changed: Condvar,
}

impl Exit {
    fn set(&self, code: Option<u32>) {
        *lock(&self.code) = Some(code);
        self.changed.notify_all();
    }

    fn done(&self) -> bool {
        lock(&self.code).is_some()
    }

    /// Whether it exited within `timeout`.
    fn wait(&self, timeout: Duration) -> bool {
        let guard = lock(&self.code);
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, timeout, |code| code.is_none())
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.is_some()
    }
}

struct Live {
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    ring: Mutex<Ring>,
    tree: ProcessTree,
    exit: Exit,
}

impl Live {
    fn not_running(agent_id: &str) -> AppError {
        AppError::not_found(format!("agent {agent_id} has no running session"))
    }
}

/// Owns every agent session. Share it as `Arc<SessionManager>`.
pub struct SessionManager {
    registry: Arc<SessionRegistry>,
    max_sessions: Box<dyn Fn() -> u32 + Send + Sync>,
    events: Arc<dyn SessionEvents>,
    /// Running sessions, and exited ones until the agent starts again (for snapshots).
    live: Mutex<HashMap<String, Arc<Live>>>,
    launcher: Option<Launcher>,
    /// Tests only: hold the exit thread just after it marks the exit, widening the window
    /// in which a closed writer matters.
    #[cfg(test)]
    exit_pause: Duration,
}

impl SessionManager {
    /// `max_sessions` is read on every start (the `maxConcurrentAgents` setting).
    pub fn new(
        registry: Arc<SessionRegistry>,
        max_sessions: impl Fn() -> u32 + Send + Sync + 'static,
        events: Arc<dyn SessionEvents>,
    ) -> Self {
        Self {
            registry,
            max_sessions: Box::new(max_sessions),
            events,
            live: Mutex::new(HashMap::new()),
            launcher: None,
            #[cfg(test)]
            exit_pause: Duration::ZERO,
        }
    }

    /// Enables [`launch`](Self::launch): `repos` resolves projects (and which are archived),
    /// `preflight` finds `claude` and checks the login (`core::claude::preflight`).
    pub fn with_launcher(
        mut self,
        repos: Arc<RepoIndex>,
        preflight: impl Fn() -> Result<ClaudeLaunch, AppError> + Send + Sync + 'static,
    ) -> Self {
        self.launcher = Some(Launcher {
            repos,
            preflight: Box::new(preflight),
        });
        self
    }

    /// Launches an agent's interactive Claude Code session in the project's repo.
    ///
    /// In order: the preflight (`claude_not_found`, `not_signed_in`, `wrong_auth_mode`), a
    /// native executable rather than a `.cmd`/`.bat` shim (`claude_needs_native`), the repo
    /// (`not_found`, `folder_missing`), then the session slot, reserved only if the
    /// project isn't archived (`project_archived`). That last check and the reservation are
    /// one step under the session registry's lock, which `archive_project` also holds, so a
    /// project can never be archived and get a new session at once. Nothing is spawned
    /// unless every check passes.
    pub fn launch(&self, request: LaunchRequest) -> Result<SessionInfo, AppError> {
        let launcher = self
            .launcher
            .as_ref()
            .ok_or_else(|| AppError::internal("agent launching isn't configured"))?;
        let claude = (launcher.preflight)()?;
        let cwd = launcher.repos.repo(&request.project_id)?;
        require_native(&claude.program)?;
        let mut args = Vec::new();
        if let Some(model) = claude.model.filter(|m| !m.trim().is_empty()) {
            args.extend(["--model".to_string(), model]);
        }
        args.push(request.launch_message);
        let spec = LaunchSpec {
            agent_id: request.agent_id,
            project_id: request.project_id.clone(),
            program: claude.program,
            args,
            cwd,
            hook: request.hook,
            cols: request.cols,
            rows: request.rows,
        };
        let repos = Arc::clone(&launcher.repos);
        self.start_checked(spec, move || {
            if repos.is_archived(&request.project_id) {
                Err(AppError::new(
                    ErrorCode::ProjectArchived,
                    "unarchive this project before starting an agent in it",
                ))
            } else {
                Ok(())
            }
        })
    }

    #[cfg(test)]
    fn with_exit_pause(mut self, pause: Duration) -> Self {
        self.exit_pause = pause;
        self
    }

    fn live(&self, agent_id: &str) -> Option<Arc<Live>> {
        lock(&self.live).get(agent_id).cloned()
    }

    /// Live sessions, ordered by agent id.
    pub fn list(&self) -> Vec<SessionInfo> {
        self.registry.list()
    }

    /// Spawns the session. `session_running` if this agent already runs, `too_many_sessions`
    /// past the cap. Returns once the process is running; `core::sessions` shows the project
    /// as active from the moment the slot is reserved.
    pub fn start(&self, spec: LaunchSpec) -> Result<SessionInfo, AppError> {
        self.start_checked(spec, || Ok(()))
    }

    /// [`start`](Self::start), with `allowed` checked atomically with the slot reservation.
    fn start_checked(
        &self,
        spec: LaunchSpec,
        allowed: impl FnOnce() -> Result<(), AppError>,
    ) -> Result<SessionInfo, AppError> {
        if spec.agent_id.trim().is_empty() || spec.project_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "agent and project ids can't be empty",
            ));
        }
        if spec.cols == 0 || spec.rows == 0 {
            return Err(AppError::invalid_input(
                "terminal size must be at least 1×1",
            ));
        }
        if !spec.cwd.is_dir() {
            return Err(AppError::new(
                ErrorCode::FolderMissing,
                format!("project folder is missing: {}", spec.cwd.display()),
            ));
        }
        self.registry.reserve_if(
            &spec.agent_id,
            &spec.project_id,
            (self.max_sessions)(),
            allowed,
        )?;
        self.events.status(AgentStatus::new(
            &spec.agent_id,
            SessionStatus::Starting,
            None,
            None,
        ));
        match self.spawn(&spec) {
            Ok(info) => Ok(info),
            Err(err) => {
                self.registry.release(&spec.agent_id);
                self.events.status(AgentStatus::new(
                    &spec.agent_id,
                    SessionStatus::Exited,
                    None,
                    None,
                ));
                tracing::warn!(agent_id = %spec.agent_id, error = %err, "agent session failed to start");
                Err(err)
            }
        }
    }

    fn spawn(&self, spec: &LaunchSpec) -> Result<SessionInfo, AppError> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| pty_error("could not open a terminal", e))?;
        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        cmd.env_clear();
        for (key, value) in child_env(spec) {
            cmd.env(key, value);
        }
        let mut child = pair.slave.spawn_command(cmd).map_err(|e| {
            AppError::new(
                ErrorCode::Io,
                format!("could not start {}: {e}", spec.program.display()),
            )
        })?;
        // The child holds its own end now; keeping ours would stop EOF reaching the reader.
        drop(pair.slave);

        let tree = match ProcessTree::attach(child.as_ref()) {
            Ok(tree) => tree,
            Err(err) => {
                let _ = child.kill();
                return Err(err);
            }
        };
        let handles = pair
            .master
            .try_clone_reader()
            .and_then(|reader| Ok((reader, pair.master.take_writer()?)));
        let (reader, writer) = match handles {
            Ok(handles) => handles,
            Err(err) => {
                let _ = tree.kill();
                return Err(pty_error("could not connect to the terminal", err));
            }
        };
        let pid = child.process_id();
        let live = Arc::new(Live {
            writer: Mutex::new(Some(writer)),
            master: Mutex::new(Some(pair.master)),
            ring: Mutex::new(Ring::new(RING_BYTES)),
            tree,
            exit: Exit::default(),
        });
        lock(&self.live).insert(spec.agent_id.clone(), Arc::clone(&live));

        let info = self
            .registry
            .set_running(&spec.agent_id, pid)
            .ok_or_else(|| AppError::internal("session vanished while starting"))?;
        self.events.status(AgentStatus::new(
            &spec.agent_id,
            SessionStatus::Running,
            None,
            pid,
        ));
        tracing::info!(agent_id = %spec.agent_id, project_id = %spec.project_id, ?pid, "agent session started");

        // Reader → coalescer → ring + events. A dedicated thread per stage: reads block.
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let agent = spec.agent_id.clone();
        let replier = Arc::clone(&live);
        spawn_named(format!("pty-read-{agent}"), move || {
            read_pty(reader, &tx, |reply| {
                if let Some(writer) = lock(&replier.writer).as_mut() {
                    let _ = writer.write_all(reply).and_then(|()| writer.flush());
                }
            });
        })?;
        let (events, ring_owner) = (Arc::clone(&self.events), Arc::clone(&live));
        let coalescer = spawn_named(format!("pty-batch-{agent}"), move || {
            coalesce(&rx, BATCH, |batch| {
                lock(&ring_owner.ring).push(batch);
                events.output(AgentOutput::new(&agent, batch));
            });
        })?;

        let (registry, events, agent) = (
            Arc::clone(&self.registry),
            Arc::clone(&self.events),
            spec.agent_id.clone(),
        );
        #[cfg(test)]
        let exit_pause = self.exit_pause;
        spawn_named(format!("pty-wait-{agent}"), move || {
            let code = child.wait().ok().map(|status| status.exit_code());
            // First, so `is_project_active` turns false the instant the process is gone.
            registry.release(&agent);
            // Before marking the exit, so a write after `stop` returns is always refused.
            lock(&live.writer).take();
            live.exit.set(code);
            #[cfg(test)]
            thread::sleep(exit_pause);
            // Closing the terminal ends the reader (ConPTY gives no EOF while it's open).
            lock(&live.master).take();
            let _ = coalescer.join();
            events.status(AgentStatus::new(&agent, SessionStatus::Exited, code, None));
            tracing::info!(agent_id = %agent, ?code, "agent session exited");
        })?;
        Ok(info)
    }

    /// Sends keystrokes (or pasted text) to the session.
    pub fn write(&self, agent_id: &str, data: &[u8]) -> Result<(), AppError> {
        let live = self
            .live(agent_id)
            .ok_or_else(|| Live::not_running(agent_id))?;
        let mut writer = lock(&live.writer);
        let writer = writer.as_mut().ok_or_else(|| Live::not_running(agent_id))?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, agent_id: &str, cols: u16, rows: u16) -> Result<(), AppError> {
        if cols == 0 || rows == 0 {
            return Err(AppError::invalid_input(
                "terminal size must be at least 1×1",
            ));
        }
        let live = self
            .live(agent_id)
            .ok_or_else(|| Live::not_running(agent_id))?;
        let master = lock(&live.master);
        let master = master.as_ref().ok_or_else(|| Live::not_running(agent_id))?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| pty_error("could not resize the terminal", e))
    }

    /// The terminal's current size, as `(cols, rows)`.
    pub fn size(&self, agent_id: &str) -> Result<(u16, u16), AppError> {
        let live = self
            .live(agent_id)
            .ok_or_else(|| Live::not_running(agent_id))?;
        let master = lock(&live.master);
        let master = master.as_ref().ok_or_else(|| Live::not_running(agent_id))?;
        let size = master
            .get_size()
            .map_err(|e| pty_error("could not read the terminal size", e))?;
        Ok((size.cols, size.rows))
    }

    /// Stops the session: Ctrl+C, then after [`STOP_GRACE`] (or at once with `force`) kills
    /// the whole process tree. Returns once the process is gone. Stopping an agent that
    /// already exited is fine.
    pub fn stop(&self, agent_id: &str, force: bool) -> Result<(), AppError> {
        let live = self
            .live(agent_id)
            .ok_or_else(|| Live::not_running(agent_id))?;
        if live.exit.done() {
            return Ok(());
        }
        if !force {
            if let Err(err) = self.write(agent_id, INTERRUPT) {
                tracing::warn!(agent_id, error = %err, "could not interrupt agent");
            }
            if live.exit.wait(STOP_GRACE) {
                tracing::info!(agent_id, "agent stopped by interrupt");
                return Ok(());
            }
        }
        live.tree.kill()?;
        if live.exit.wait(KILL_WAIT) {
            tracing::info!(agent_id, force, "agent process tree killed");
            Ok(())
        } else {
            Err(AppError::internal(format!(
                "agent {agent_id} is still running after being killed"
            )))
        }
    }

    /// The newest output (at most [`RING_BYTES`]), for a terminal that remounts. Empty if
    /// the agent hasn't run.
    pub fn snapshot(&self, agent_id: &str) -> Vec<u8> {
        self.live(agent_id)
            .map(|live| lock(&live.ring).snapshot())
            .unwrap_or_default()
    }

    /// Kills every session's process tree and waits briefly for each. For app exit.
    pub fn shutdown(&self) {
        let all: Vec<(String, Arc<Live>)> = lock(&self.live)
            .iter()
            .map(|(id, live)| (id.clone(), Arc::clone(live)))
            .collect();
        for (agent_id, live) in &all {
            if !live.exit.done() {
                if let Err(err) = live.tree.kill() {
                    tracing::warn!(%agent_id, error = %err, "could not kill agent on exit");
                }
            }
        }
        for (agent_id, live) in &all {
            if !live.exit.wait(KILL_WAIT) {
                tracing::warn!(%agent_id, "agent still running at exit");
            }
        }
        tracing::info!(sessions = all.len(), "agent sessions shut down");
    }
}

fn spawn_named(
    name: String,
    f: impl FnOnce() + Send + 'static,
) -> Result<thread::JoinHandle<()>, AppError> {
    thread::Builder::new()
        .name(name)
        .spawn(f)
        .map_err(|e| AppError::internal(format!("could not start a session thread: {e}")))
}

/// Reads the terminal until it closes, forwarding chunks to `tx`. Answers ConPTY's startup
/// cursor query itself (via `reply`) and drops it from the stream, so a session starts even
/// with no terminal view open, and a view mounted later doesn't answer it a second time.
fn read_pty(
    mut reader: Box<dyn Read + Send>,
    tx: &mpsc::Sender<Vec<u8>>,
    mut reply: impl FnMut(&[u8]),
) {
    let mut startup = StartupQuery::default();
    let mut buf = vec![0u8; READ_CHUNK];
    loop {
        let chunk = match reader.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => startup.filter(&buf[..n], &mut reply),
        };
        if !chunk.is_empty() && tx.send(chunk).is_err() {
            return;
        }
    }
}

/// Spots a cursor query at the very start of the output (possibly split across reads).
#[derive(Default)]
struct StartupQuery {
    held: Vec<u8>,
    done: bool,
}

impl StartupQuery {
    fn filter(&mut self, chunk: &[u8], reply: &mut impl FnMut(&[u8])) -> Vec<u8> {
        if self.done {
            return chunk.to_vec();
        }
        self.held.extend_from_slice(chunk);
        if self.held.starts_with(CURSOR_QUERY) {
            self.done = true;
            reply(CURSOR_REPLY);
            return self.held.split_off(CURSOR_QUERY.len());
        }
        if CURSOR_QUERY.starts_with(&self.held) {
            return Vec::new(); // could still become the query: wait for more
        }
        self.done = true;
        std::mem::take(&mut self.held)
    }
}

/// Groups chunks arriving within `window` of the first into one batch (flushed early at
/// [`MAX_BATCH_BYTES`]). Returns when the sender is gone, after flushing what's left.
fn coalesce(rx: &Receiver<Vec<u8>>, window: Duration, mut flush: impl FnMut(&[u8])) {
    while let Ok(mut batch) = rx.recv() {
        let deadline = Instant::now() + window;
        let mut open = true;
        while batch.len() < MAX_BATCH_BYTES {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            match rx.recv_timeout(left) {
                Ok(more) => batch.extend_from_slice(&more),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    open = false;
                    break;
                }
            }
        }
        flush(&batch);
        if !open {
            return;
        }
    }
}

#[cfg(test)]
mod tests;
