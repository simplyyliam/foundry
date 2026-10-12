use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;

use super::*;
use crate::core::process_tree::pid_alive;
use crate::testing::{fake_agent_path, TestRepo};

const PROJECT: &str = "project-1";
const OTHER_PROJECT: &str = "project-2";

// ---------- fixtures ----------

/// Records every event, decoded, and wakes waiters.
struct Recorder {
    registry: Arc<SessionRegistry>,
    output: Mutex<Vec<Vec<u8>>>,
    /// The same batches, with the project each event named.
    output_in: Mutex<Vec<(String, Vec<u8>)>>,
    statuses: Mutex<Vec<AgentStatus>>,
    /// For each `exited` event: was the project still active when it was emitted?
    active_at_exit: Mutex<Vec<bool>>,
    changed: Condvar,
}

impl SessionEvents for Recorder {
    fn output(&self, event: AgentOutput) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(event.chunk)
            .unwrap();
        self.output_in
            .lock()
            .unwrap()
            .push((event.project_id, bytes.clone()));
        self.output.lock().unwrap().push(bytes);
        self.changed.notify_all();
    }

    fn status(&self, event: AgentStatus) {
        if event.status == SessionStatus::Exited {
            self.active_at_exit
                .lock()
                .unwrap()
                .push(self.registry.is_project_active(PROJECT));
        }
        self.statuses.lock().unwrap().push(event);
        self.changed.notify_all();
    }
}

impl Recorder {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().unwrap().concat()).into_owned()
    }

    fn batches(&self) -> usize {
        self.output.lock().unwrap().len()
    }

    /// The output of one project's sessions.
    fn text_in(&self, project_id: &str) -> String {
        let bytes: Vec<u8> = self
            .output_in
            .lock()
            .unwrap()
            .iter()
            .filter(|(project, _)| project == project_id)
            .flat_map(|(_, bytes)| bytes.clone())
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Waits until `ok` holds for the recorded output text.
    fn wait_text(&self, timeout: Duration, ok: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + timeout;
        let mut guard = self.output.lock().unwrap();
        loop {
            let text = String::from_utf8_lossy(&guard.concat()).into_owned();
            if ok(&text) {
                return text;
            }
            let left = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("timed out waiting for output; got:\n{text}"));
            guard = self.changed.wait_timeout(guard, left).unwrap().0;
        }
    }

    /// Waits for the agent's `exited` event in [`PROJECT`] and returns it.
    fn wait_exited(&self, agent_id: &str, timeout: Duration) -> AgentStatus {
        self.wait_exited_in(PROJECT, agent_id, timeout)
    }

    fn wait_exited_in(&self, project_id: &str, agent_id: &str, timeout: Duration) -> AgentStatus {
        let deadline = Instant::now() + timeout;
        let mut guard = self.statuses.lock().unwrap();
        loop {
            if let Some(event) = guard.iter().find(|e| {
                e.project_id == project_id
                    && e.agent_id == agent_id
                    && e.status == SessionStatus::Exited
            }) {
                return event.clone();
            }
            let left = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("{project_id}/{agent_id} never exited"));
            guard = self.changed.wait_timeout(guard, left).unwrap().0;
        }
    }

    fn statuses_of(&self, agent_id: &str) -> Vec<SessionStatus> {
        self.statuses_in(PROJECT, agent_id)
    }

    fn statuses_in(&self, project_id: &str, agent_id: &str) -> Vec<SessionStatus> {
        self.statuses
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.project_id == project_id && e.agent_id == agent_id)
            .map(|e| e.status)
            .collect()
    }
}

struct Env {
    repo: TestRepo,
    registry: Arc<SessionRegistry>,
    events: Arc<Recorder>,
    manager: Arc<SessionManager>,
}

impl Env {
    fn new(max: u32) -> Self {
        Self::with_exit_pause(max, Duration::ZERO)
    }

    fn with_exit_pause(max: u32, pause: Duration) -> Self {
        let registry = Arc::new(SessionRegistry::default());
        let events = Arc::new(Recorder {
            registry: Arc::clone(&registry),
            output: Mutex::default(),
            output_in: Mutex::default(),
            statuses: Mutex::default(),
            active_at_exit: Mutex::default(),
            changed: Condvar::new(),
        });
        let manager = Arc::new(
            SessionManager::new(
                Arc::clone(&registry),
                move || max,
                Arc::clone(&events) as Arc<dyn SessionEvents>,
            )
            .with_exit_pause(pause),
        );
        Self {
            repo: TestRepo::new(),
            registry,
            events,
            manager,
        }
    }

    fn spec(&self, agent_id: &str, args: &[&str]) -> LaunchSpec {
        self.spec_in(PROJECT, agent_id, args)
    }

    fn spec_in(&self, project_id: &str, agent_id: &str, args: &[&str]) -> LaunchSpec {
        LaunchSpec {
            agent_id: agent_id.into(),
            project_id: project_id.into(),
            program: fake_agent_path(),
            args: args.iter().map(|a| a.to_string()).collect(),
            cwd: self.repo.path().to_path_buf(),
            hook: HookEnv {
                port: 4567,
                token: "tok-123".into(),
            },
            slice_id: None,
            workspace_access: false,
            cols: 200,
            rows: 50,
        }
    }

    fn start(&self, agent_id: &str, args: &[&str]) -> SessionInfo {
        self.manager.start(self.spec(agent_id, args)).unwrap()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        self.manager.shutdown();
    }
}

const LONG: Duration = Duration::from_secs(20);

fn wait_until(timeout: Duration, ok: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    ok()
}

fn child_pid(text: &str) -> u32 {
    let start = text.find("child-pid=").expect("child pid printed") + "child-pid=".len();
    text[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

// ---------- R-7-1 ----------

#[test]
fn spawned_env_has_no_api_or_provider_variables_and_has_foundrys() {
    // Set in this test process: the session must not inherit them.
    std::env::set_var("ANTHROPIC_API_KEY", "sk-should-not-leak");
    std::env::set_var("ANTHROPIC_BASE_URL", "https://example.invalid");
    std::env::set_var("CLAUDE_CODE_USE_BEDROCK", "1");
    std::env::set_var("FOUNDRY_HOOK_TOKEN", "stale-token");
    std::env::set_var("COLORTERM", "stale-colorterm");
    let env = Env::new(3);
    let mut spec = env.spec("builder", &["--print-env", "--print-cwd"]);
    spec.cols = 2000; // no wrapping of long values such as PATH
    env.manager.start(spec).unwrap();
    env.events.wait_exited("builder", LONG);
    let text = env.events.text();

    for wanted in [
        "FOUNDRY_AGENT_ID=builder",
        "FOUNDRY_PROJECT_ID=project-1",
        "FOUNDRY_HOOK_PORT=4567",
        "FOUNDRY_HOOK_TOKEN=tok-123",
        "TERM=xterm-256color",
        "COLORTERM=truecolor",
    ] {
        assert!(text.contains(wanted), "missing {wanted} in:\n{text}");
    }
    for unwanted in [
        "ANTHROPIC_",
        "CLAUDE_CODE_USE_",
        "sk-should-not-leak",
        "stale-token",
        "stale-colorterm",
    ] {
        assert!(!text.contains(unwanted), "{unwanted} leaked into:\n{text}");
    }
    let folder = env
        .repo
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(text.contains("cwd="), "{text}");
    assert!(text.contains(&folder), "runs in the linked repo:\n{text}");
}

// ---------- R-7-2 ----------

#[test]
fn output_streams_and_coalesces_a_burst() {
    let env = Env::new(3);
    env.start("builder", &["--burst", "300"]);
    env.events.wait_exited("builder", LONG);
    let text = env.events.text();

    let mut at = 0;
    for i in 0..300 {
        let line = format!("burst-{i:05}");
        let found = text[at..]
            .find(&line)
            .unwrap_or_else(|| panic!("{line} missing or out of order"));
        at += found + line.len();
    }
    let batches = env.events.batches();
    assert!(
        batches < 100,
        "300 flushed lines arrived in {batches} batches"
    );
}

#[test]
fn coalesce_groups_chunks_within_the_window_and_splits_after_it() {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let feeder = thread::spawn(move || {
        for _ in 0..50 {
            tx.send(b"a".to_vec()).unwrap();
        }
        thread::sleep(Duration::from_millis(80));
        tx.send(b"b".to_vec()).unwrap();
    });
    let mut batches: Vec<Vec<u8>> = Vec::new();
    coalesce(&rx, BATCH, |batch| batches.push(batch.to_vec()));
    feeder.join().unwrap();
    assert_eq!(batches, vec![vec![b'a'; 50], b"b".to_vec()]);
}

#[test]
fn coalesce_flushes_early_once_a_batch_is_large() {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    for _ in 0..3 {
        tx.send(vec![b'x'; MAX_BATCH_BYTES / 2]).unwrap();
    }
    drop(tx);
    let mut sizes = Vec::new();
    coalesce(&rx, Duration::from_secs(5), |batch| sizes.push(batch.len()));
    assert_eq!(sizes, [MAX_BATCH_BYTES, MAX_BATCH_BYTES / 2]);
}

#[test]
fn the_startup_cursor_query_is_answered_and_dropped_even_when_split() {
    let mut replies: Vec<Vec<u8>> = Vec::new();
    let mut query = StartupQuery::default();
    let mut out = Vec::new();
    for chunk in [&b"\x1b["[..], b"6nhello", b"\x1b[6n again"] {
        out.extend(query.filter(chunk, &mut |r: &[u8]| replies.push(r.to_vec())));
    }
    assert_eq!(replies, [CURSOR_REPLY.to_vec()], "answered once");
    assert_eq!(
        out, b"hello\x1b[6n again",
        "later queries pass through to the terminal"
    );

    let mut plain = StartupQuery::default();
    let mut none = 0;
    let out = plain.filter(b"\x1b[?25l text", &mut |_: &[u8]| none += 1);
    assert_eq!((out.as_slice(), none), (&b"\x1b[?25l text"[..], 0));
}

// ---------- R-7-3 ----------

#[test]
fn input_written_with_agent_write_reaches_the_process() {
    let env = Env::new(3);
    env.start("builder", &["--echo-upper"]);

    env.manager
        .write(PROJECT, "builder", b"hello foundry\r\n")
        .unwrap();

    // Only the process itself turns the line into upper case.
    env.events
        .wait_text(LONG, |text| text.contains("got:HELLO FOUNDRY"));
    env.manager.stop(PROJECT, "builder", true).unwrap();
    assert_eq!(
        env.manager
            .write(PROJECT, "builder", b"x")
            .unwrap_err()
            .code,
        ErrorCode::NotFound,
        "writing after exit is refused"
    );
}

/// The writer closes before the exit is announced, so a write after `stop` returns, or after
/// `exited` fires, is always refused (`not_found`) rather than silently swallowed.
#[test]
fn writes_after_an_exit_is_observed_are_always_refused() {
    // The pause holds the exit thread right after it marks the exit: if the writer were
    // still open then (the old order), these writes would succeed.
    let env = Env::with_exit_pause(3, Duration::from_millis(200));
    for round in 0..20 {
        let agent = format!("stopped-{round}");
        env.start(&agent, &["--echo-upper"]);
        env.manager.stop(PROJECT, &agent, true).unwrap();
        let err = env
            .manager
            .write(
                PROJECT, &agent, b"late
",
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound, "round {round}: {err}");
    }

    // The process exits by itself; the app reacts to `exited` with a write.
    env.start("quitter", &["--exit", "0"]);
    env.events.wait_exited("quitter", LONG);
    assert_eq!(
        env.manager
            .write(
                PROJECT, "quitter", b"late
"
            )
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

/// Writes racing the exit itself either land (before the writer closes) or are refused:
/// never another error, and never a hang.
#[test]
fn writes_racing_an_exit_only_succeed_or_report_not_found() {
    let env = Env::new(3);
    for round in 0..10 {
        let agent = format!("racer-{round}");
        env.start(&agent, &["--sleep-ms", "50"]);
        let deadline = Instant::now() + LONG;
        loop {
            match env.manager.write(PROJECT, &agent, b"x") {
                Ok(()) => assert!(Instant::now() < deadline, "round {round}: never exited"),
                Err(err) => {
                    assert_eq!(err.code, ErrorCode::NotFound, "round {round}: {err}");
                    break;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
        env.events.wait_exited(&agent, LONG);
    }
}

// ---------- R-7-4 ----------

#[test]
fn agent_resize_changes_the_pty_size() {
    let env = Env::new(3);
    env.start("builder", &["--echo-upper"]);
    assert_eq!(env.manager.size(PROJECT, "builder").unwrap(), (200, 50));

    env.manager.resize(PROJECT, "builder", 120, 40).unwrap();

    assert_eq!(env.manager.size(PROJECT, "builder").unwrap(), (120, 40));
    assert_eq!(
        env.manager
            .resize(PROJECT, "builder", 0, 40)
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}

/// The program inside the terminal sees the new size (not just our bookkeeping).
#[test]
fn a_resized_terminal_reports_its_new_size_to_the_program() {
    let env = Env::new(3);
    let mut spec = env.spec("shell", &[]);
    if cfg!(windows) {
        spec.program = "cmd.exe".into();
        spec.args = vec!["/Q".into(), "/K".into()];
    } else {
        spec.program = "sh".into();
    }
    env.manager.start(spec).unwrap();

    env.manager.resize(PROJECT, "shell", 132, 43).unwrap();
    let probe: &[u8] = if cfg!(windows) {
        b"mode con\r\n"
    } else {
        b"stty size\n"
    };
    env.manager.write(PROJECT, "shell", probe).unwrap();

    if cfg!(windows) {
        env.events.wait_text(LONG, |text| {
            text.lines()
                .any(|line| line.contains("Columns:") && line.contains("132"))
        });
    } else {
        env.events.wait_text(LONG, |text| text.contains("43 132"));
    }
    env.manager.stop(PROJECT, "shell", true).unwrap();
}

// ---------- R-7-5 ----------

#[test]
fn agent_stop_ends_the_process_and_its_child() {
    let env = Env::new(3);
    let info = env.start(
        "builder",
        &["--spawn-child", "60000", "--sleep-ms", "60000"],
    );
    let parent = info.pid.unwrap();
    let child = child_pid(
        &env.events
            .wait_text(LONG, |t| t.contains("child-pid=") && t.contains('\n')),
    );
    assert!(pid_alive(parent) && pid_alive(child));

    env.manager.stop(PROJECT, "builder", false).unwrap();

    assert!(!pid_alive(parent), "agent still running");
    assert!(
        wait_until(Duration::from_secs(3), || !pid_alive(child)),
        "child still running"
    );
    env.events.wait_exited("builder", LONG);
    let listed = env.manager.list(PROJECT);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, SessionStatus::Exited, "kept as finished");
    assert!(!env.registry.is_project_active(PROJECT));
}

#[test]
fn force_stop_kills_the_whole_tree_at_once() {
    let env = Env::new(3);
    let info = env.start(
        "builder",
        &["--spawn-child", "60000", "--sleep-ms", "60000"],
    );
    let parent = info.pid.unwrap();
    let child = child_pid(
        &env.events
            .wait_text(LONG, |t| t.contains("child-pid=") && t.contains('\n')),
    );

    let started = Instant::now();
    env.manager.stop(PROJECT, "builder", true).unwrap();

    assert!(
        started.elapsed() < STOP_GRACE,
        "force skips the grace period"
    );
    assert!(!pid_alive(parent));
    assert!(
        wait_until(Duration::from_secs(3), || !pid_alive(child)),
        "child still running"
    );
    let exited = env.events.wait_exited("builder", LONG);
    assert_eq!(exited.exit_code, Some(1), "killed by the job object");
}

/// A program that ignores Ctrl+C (an idle `cmd.exe` just prints a new prompt) is killed
/// after the grace period.
#[cfg(windows)]
#[test]
fn stop_kills_after_the_grace_period_when_the_interrupt_is_ignored() {
    let env = Env::new(3);
    let mut spec = env.spec("shell", &[]);
    spec.program = "cmd.exe".into();
    // A custom prompt that only appears once cmd.exe is in its interactive input loop, the
    // state where it ignores Ctrl+C. Before that, Ctrl+C can end it during startup.
    spec.args = vec!["/Q".into(), "/K".into(), "prompt FOUNDRY-READY$G".into()];
    let pid = env.manager.start(spec).unwrap().pid.unwrap();
    env.events
        .wait_text(LONG, |text| text.contains("FOUNDRY-READY>"));

    let started = Instant::now();
    env.manager.stop(PROJECT, "shell", false).unwrap();

    let took = started.elapsed();
    assert!(
        took >= STOP_GRACE,
        "killed after {took:?}, before the grace period ended"
    );
    assert!(took < STOP_GRACE + Duration::from_secs(4), "took {took:?}");
    assert!(!pid_alive(pid));
}

// ---------- R-7-6 ----------

#[test]
fn the_session_cap_is_enforced() {
    let env = Env::new(2);
    env.start("a", &["--sleep-ms", "60000"]);
    env.start("b", &["--sleep-ms", "60000"]);

    let refused = env
        .manager
        .start(env.spec("c", &["--sleep-ms", "60000"]))
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::TooManySessions);
    assert!(
        env.events.statuses_of("c").is_empty(),
        "nothing started for c"
    );
    let again = env
        .manager
        .start(env.spec("a", &["--sleep-ms", "60000"]))
        .unwrap_err();
    assert_eq!(again.code, ErrorCode::SessionRunning);
    assert_eq!(env.manager.list(PROJECT).len(), 2);

    env.manager.stop(PROJECT, "a", true).unwrap();
    // a's finished session is still listed, but doesn't take one of the two slots.
    env.start("c", &["--sleep-ms", "60000"]);
    let listed: Vec<(String, SessionStatus)> = env
        .manager
        .list(PROJECT)
        .into_iter()
        .map(|s| (s.agent_id, s.status))
        .collect();
    assert_eq!(
        listed,
        [
            ("a".to_string(), SessionStatus::Exited),
            ("b".to_string(), SessionStatus::Running),
            ("c".to_string(), SessionStatus::Running)
        ]
    );
}

// ---------- R-7-7 ----------

#[test]
fn the_ring_buffer_is_bounded_and_snapshot_returns_the_latest_bytes() {
    let env = Env::new(3);
    // One bulk write (~360 KB, past the ring) instead of 30 000 flushed lines: per-line
    // flushes through ConPTY took seconds alone and over a minute under a parallel suite.
    env.start("builder", &["--burst-block", "30000"]);
    // Woken on each status event, so it returns as soon as the exit lands; the deadline only
    // bounds a hang.
    env.events.wait_exited("builder", Duration::from_secs(180));
    let all = env.events.output.lock().unwrap().concat();
    assert!(
        all.len() > RING_BYTES,
        "burst produced only {} bytes",
        all.len()
    );

    let snapshot = env.manager.snapshot(PROJECT, "builder");

    assert_eq!(snapshot.len(), RING_BYTES);
    assert_eq!(
        snapshot,
        all[all.len() - RING_BYTES..],
        "exactly the newest bytes"
    );
    assert!(String::from_utf8_lossy(&snapshot).contains("burst-29999"));
    assert!(env.manager.snapshot(PROJECT, "never-ran").is_empty());
}

// ---------- R-7-8 ----------

#[test]
fn shutdown_leaves_no_child_processes() {
    let env = Env::new(3);
    let mut pids = Vec::new();
    for agent in ["a", "b"] {
        pids.push(
            env.start(agent, &["--spawn-child", "60000", "--sleep-ms", "60000"])
                .pid
                .unwrap(),
        );
    }
    let text = env.events.wait_text(LONG, |t| {
        t.matches("child-pid=").count() == 2 && t.ends_with('\n')
    });
    let mut rest = text.as_str();
    while let Some(at) = rest.find("child-pid=") {
        pids.push(child_pid(&rest[at..]));
        rest = &rest[at + 1..];
    }
    assert_eq!(pids.len(), 4);
    assert!(pids.iter().all(|&pid| pid_alive(pid)));

    env.manager.shutdown();

    assert!(
        wait_until(Duration::from_secs(3), || pids
            .iter()
            .all(|&pid| !pid_alive(pid))),
        "still alive: {:?}",
        pids.iter()
            .filter(|&&pid| pid_alive(pid))
            .collect::<Vec<_>>()
    );
}

// ---------- R-7-10 ----------

#[test]
fn project_is_active_from_start_until_the_process_exits() {
    let env = Env::new(3);
    assert!(!env.registry.is_project_active(PROJECT));

    let info = env.start("builder", &["--sleep-ms", "400"]);

    assert!(
        env.registry.is_project_active(PROJECT),
        "true as soon as start returns"
    );
    assert_eq!(info.status, SessionStatus::Running);
    let pid = info.pid.unwrap();
    assert!(wait_until(LONG, || !pid_alive(pid)));
    // Released before anything else happens on exit: already false when `exited` fires.
    env.events.wait_exited("builder", LONG);
    assert_eq!(*env.events.active_at_exit.lock().unwrap(), [false]);
    assert!(!env.registry.is_project_active(PROJECT));
    assert_eq!(
        env.events.statuses_of("builder"),
        [
            SessionStatus::Starting,
            SessionStatus::Running,
            SessionStatus::Exited
        ]
    );
}

#[test]
fn a_failed_spawn_releases_its_slot() {
    let env = Env::new(1);
    let mut spec = env.spec("builder", &[]);
    spec.program = env.repo.path().join("no-such-program.exe");

    let err = env.manager.start(spec).unwrap_err();

    assert_eq!(err.code, ErrorCode::Io);
    assert!(!env.registry.is_project_active(PROJECT));
    assert_eq!(
        env.events.statuses_of("builder"),
        [SessionStatus::Starting, SessionStatus::Exited]
    );
    env.start("other", &["--sleep-ms", "10"]); // the one slot is free again
}

#[test]
fn start_validates_its_spec() {
    let env = Env::new(3);
    let mut missing = env.spec("builder", &[]);
    missing.cwd = env.repo.path().join("gone");
    assert_eq!(
        env.manager.start(missing).unwrap_err().code,
        ErrorCode::FolderMissing
    );
    let mut tiny = env.spec("builder", &[]);
    tiny.cols = 0;
    assert_eq!(
        env.manager.start(tiny).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert!(env.manager.list(PROJECT).is_empty());
}

// ---------- sessions are per (project, agent) ----------

#[test]
fn the_same_agent_id_in_two_projects_gets_two_sessions_and_stopping_one_leaves_the_other() {
    let env = Env::new(3);
    let one = env.start("builder", &["--echo-upper"]);
    let two = env
        .manager
        .start(env.spec_in(OTHER_PROJECT, "builder", &["--echo-upper"]))
        .unwrap();
    assert_ne!(one.pid, two.pid);
    assert_eq!(
        (env.manager.list(PROJECT), env.manager.list(OTHER_PROJECT)),
        (
            vec![env.registry.list()[0].clone()],
            vec![env.registry.list()[1].clone()]
        ),
        "each project lists only its own session"
    );
    assert_eq!(env.manager.list(OTHER_PROJECT)[0].project_id, OTHER_PROJECT);

    // Input and output stay with their session, and the events say which project.
    env.manager
        .write(OTHER_PROJECT, "builder", b"second\r\n")
        .unwrap();
    assert!(wait_until(LONG, || env
        .events
        .text_in(OTHER_PROJECT)
        .contains("got:SECOND")));
    assert!(!env.events.text_in(PROJECT).contains("got:SECOND"));
    assert!(
        String::from_utf8_lossy(&env.manager.snapshot(OTHER_PROJECT, "builder"))
            .contains("got:SECOND")
    );
    assert!(
        !String::from_utf8_lossy(&env.manager.snapshot(PROJECT, "builder")).contains("got:SECOND")
    );

    env.manager.stop(PROJECT, "builder", true).unwrap();

    env.events.wait_exited_in(PROJECT, "builder", LONG);
    assert_eq!(env.manager.list(PROJECT)[0].status, SessionStatus::Exited);
    assert_eq!(
        env.manager.list(OTHER_PROJECT)[0].status,
        SessionStatus::Running,
        "the other one runs on"
    );
    assert!(pid_alive(two.pid.unwrap()));
    assert_eq!(
        env.manager
            .write(PROJECT, "builder", b"x")
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    env.manager
        .write(OTHER_PROJECT, "builder", b"still\r\n")
        .unwrap();
    assert!(wait_until(LONG, || env
        .events
        .text_in(OTHER_PROJECT)
        .contains("got:STILL")));
    assert_eq!(
        env.events.statuses_in(OTHER_PROJECT, "builder"),
        [SessionStatus::Starting, SessionStatus::Running],
        "no exit reported for the other project"
    );

    // One session per agent per project still holds.
    assert_eq!(
        env.manager
            .start(env.spec_in(OTHER_PROJECT, "builder", &["--echo-upper"]))
            .unwrap_err()
            .code,
        ErrorCode::SessionRunning
    );
    env.manager.stop(OTHER_PROJECT, "builder", true).unwrap();
    env.events.wait_exited_in(OTHER_PROJECT, "builder", LONG);

    // Each project keeps its own finished session and its own final output.
    for project in [PROJECT, OTHER_PROJECT] {
        let listed = env.manager.list(project);
        assert_eq!(listed.len(), 1, "{project}");
        assert_eq!(
            (listed[0].project_id.as_str(), listed[0].status),
            (project, SessionStatus::Exited)
        );
    }
    let other =
        String::from_utf8_lossy(&env.manager.snapshot(OTHER_PROJECT, "builder")).into_owned();
    assert!(other.contains("got:SECOND") && other.contains("got:STILL"));
    assert!(
        !String::from_utf8_lossy(&env.manager.snapshot(PROJECT, "builder")).contains("got:STILL")
    );
}

// ---------- finished sessions ----------

#[test]
fn a_finished_session_is_listed_as_exited_with_its_code_and_keeps_its_output() {
    let env = Env::new(3);
    let started = env.start("builder", &["--burst", "3", "--exit", "7"]);
    assert_eq!(started.exit_code, None, "null while it runs");

    let exited = env.events.wait_exited("builder", LONG);

    assert_eq!(exited.exit_code, Some(7));
    let listed = env.manager.list(PROJECT);
    assert_eq!(listed.len(), 1);
    let info = &listed[0];
    assert_eq!(
        (
            info.agent_id.as_str(),
            info.project_id.as_str(),
            info.status
        ),
        ("builder", PROJECT, SessionStatus::Exited)
    );
    assert_eq!((info.exit_code, info.pid), (Some(7), None));
    assert!(
        String::from_utf8_lossy(&env.manager.snapshot(PROJECT, "builder")).contains("burst-00002")
    );

    // Finished: nothing to write to, resize or stop.
    for err in [
        env.manager.write(PROJECT, "builder", b"x").unwrap_err(),
        env.manager.resize(PROJECT, "builder", 80, 24).unwrap_err(),
        env.manager.stop(PROJECT, "builder", false).unwrap_err(),
        env.manager.stop(PROJECT, "builder", true).unwrap_err(),
    ] {
        assert_eq!(err.code, ErrorCode::NotFound, "{err}");
    }
    assert!(!env.registry.is_project_active(PROJECT), "never live again");
}

#[test]
fn a_finished_session_takes_no_slot_and_is_cleared_when_the_agent_starts_again() {
    let env = Env::new(1);
    env.start("builder", &["--burst", "1", "--exit", "0"]);
    env.events.wait_exited("builder", LONG);
    assert_eq!(env.manager.list(PROJECT)[0].exit_code, Some(0));

    // The cap is 1: the finished session doesn't hold it, nor block the same agent.
    let again = env.start("builder", &["--echo-upper"]);
    assert_eq!(again.status, SessionStatus::Running);
    let listed = env.manager.list(PROJECT);
    assert_eq!(
        listed.len(),
        1,
        "the finished one is replaced, not listed twice"
    );
    assert_eq!(
        (listed[0].status, listed[0].exit_code),
        (SessionStatus::Running, None)
    );
    assert!(
        !String::from_utf8_lossy(&env.manager.snapshot(PROJECT, "builder")).contains("burst-00000"),
        "the old output is cleared too"
    );
    env.manager.stop(PROJECT, "builder", true).unwrap();
    env.events.wait_exited("builder", LONG);

    // Another agent can take the one slot while builder's finished session is listed.
    env.start("designer", &["--sleep-ms", "60000"]);
    let listed: Vec<(String, SessionStatus)> = env
        .manager
        .list(PROJECT)
        .into_iter()
        .map(|s| (s.agent_id, s.status))
        .collect();
    assert_eq!(
        listed,
        [
            ("builder".to_string(), SessionStatus::Exited),
            ("designer".to_string(), SessionStatus::Running)
        ]
    );
}

/// The cap counts sessions in every project.
#[test]
fn the_session_cap_counts_every_project() {
    let env = Env::new(2);
    env.start("a", &["--sleep-ms", "60000"]);
    env.manager
        .start(env.spec_in(OTHER_PROJECT, "a", &["--sleep-ms", "60000"]))
        .unwrap();
    let refused = env
        .manager
        .start(env.spec_in(OTHER_PROJECT, "b", &["--sleep-ms", "60000"]))
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::TooManySessions);
}

// ---------- commands over IPC ----------

#[test]
fn commands_round_trip_over_ipc() {
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::{Manager, WebviewWindowBuilder};

    let env = Env::new(3);
    // A manager that can launch: this repo as PROJECT, fake-claude as `claude`.
    let bin = TestRepo::new();
    let claude = crate::testing::install_fake_claude(bin.path());
    let repos = Arc::new(RepoIndex::default());
    repos.replace_all([(PROJECT, env.repo.path())]);
    let endpoint = Arc::new(HookEndpointCell::default());
    endpoint.publish(crate::core::hook::HookEndpoint {
        port: 4567,
        token: "tok-ipc".into(),
    });
    let manager = Arc::new(
        SessionManager::new(
            Arc::clone(&env.registry),
            || 3,
            Arc::clone(&env.events) as Arc<dyn SessionEvents>,
        )
        .with_launcher(repos, endpoint, move || {
            Ok(ClaudeLaunch {
                program: claude.clone(),
                model: None,
            })
        }),
    );
    env.repo.write(
        "agents/roles.json",
        r#"{"version":1,"agents":[{"id":"designer","displayName":"Dee","title":"Designer"}]}"#,
    );
    env.repo
        .write(".foundry/prompts/handoff.md", "# Hand-off\n");
    manager
        .start(env.spec("builder", &["--echo-upper"]))
        .unwrap();

    let app = crate::app_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Arc::clone(&manager));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, body: serde_json::Value| {
        tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<serde_json::Value>().unwrap())
    };
    let list = |project: &str| invoke("agent_list", serde_json::json!({ "projectId": project }));

    let listed = list(PROJECT).unwrap();
    assert_eq!(listed[0]["agentId"], "builder");
    assert_eq!(listed[0]["projectId"], PROJECT);
    assert_eq!(listed[0]["status"], "running");
    assert_eq!(list(OTHER_PROJECT).unwrap(), serde_json::json!([]));

    invoke(
        "agent_write",
        serde_json::json!({ "projectId": PROJECT, "agentId": "builder", "data": "ipc\r\n" }),
    )
    .unwrap();
    env.events.wait_text(LONG, |text| text.contains("got:IPC"));
    invoke(
        "agent_resize",
        serde_json::json!({ "projectId": PROJECT, "agentId": "builder", "cols": 90, "rows": 30 }),
    )
    .unwrap();
    assert_eq!(manager.size(PROJECT, "builder").unwrap(), (90, 30));
    let snapshot = invoke(
        "agent_snapshot",
        serde_json::json!({ "projectId": PROJECT, "agentId": "builder" }),
    )
    .unwrap();
    let snapshot = base64::engine::general_purpose::STANDARD
        .decode(snapshot.as_str().unwrap())
        .unwrap();
    assert!(String::from_utf8_lossy(&snapshot).contains("got:IPC"));
    // The same agent id in another project is another session: nothing to write to.
    let elsewhere = invoke(
        "agent_write",
        serde_json::json!({ "projectId": OTHER_PROJECT, "agentId": "builder", "data": "x" }),
    )
    .unwrap_err();
    assert_eq!(elsewhere["code"], "not_found");

    // agent_start: `sliceId` may be left out; the backend builds the message.
    let refused = invoke(
        "agent_start",
        serde_json::json!({ "projectId": PROJECT, "agentId": "designer",
                            "options": { "promptFile": "AGENTS.md" } }),
    )
    .unwrap_err();
    assert_eq!(refused["code"], "invalid_input");
    let started = invoke(
        "agent_start",
        serde_json::json!({ "projectId": PROJECT, "agentId": "designer",
                            "options": { "promptFile": ".foundry/prompts/handoff.md",
                                         "sliceId": "s-1" } }),
    )
    .unwrap();
    assert_eq!(started["projectId"], PROJECT);
    assert_eq!(started["agentId"], "designer");
    assert_eq!(started["status"], "running");
    env.events.wait_text(LONG, |text| text.contains("ready"));
    assert_eq!(list(PROJECT).unwrap().as_array().unwrap().len(), 2);

    for agent in ["builder", "designer"] {
        invoke(
            "agent_stop",
            serde_json::json!({ "projectId": PROJECT, "agentId": agent, "options": { "force": true } }),
        )
        .unwrap();
    }
    // Both stay listed as finished, with an exit code field.
    let listed = list(PROJECT).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 2);
    for session in listed.as_array().unwrap() {
        assert_eq!(session["status"], "exited");
        assert!(session.get("exitCode").is_some(), "{session}");
    }
    let finished = invoke(
        "agent_stop",
        serde_json::json!({ "projectId": PROJECT, "agentId": "builder", "options": { "force": true } }),
    )
    .unwrap_err();
    assert_eq!(finished["code"], "not_found");

    // No promptFile: the agent opens and waits for instructions. That clears its finished
    // session.
    let opened = invoke(
        "agent_start",
        serde_json::json!({ "projectId": PROJECT, "agentId": "designer", "options": {} }),
    )
    .unwrap();
    assert_eq!(opened["status"], "running");
    assert_eq!(opened["exitCode"], serde_json::Value::Null);
    let statuses: Vec<(String, String)> = list(PROJECT)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["agentId"].as_str().unwrap().to_string(),
                s["status"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        statuses,
        [
            ("builder".to_string(), "exited".to_string()),
            ("designer".to_string(), "running".to_string())
        ]
    );
    manager.stop(PROJECT, "designer", true).unwrap();

    let missing = invoke(
        "agent_stop",
        serde_json::json!({ "projectId": PROJECT, "agentId": "nobody", "options": { "force": false } }),
    )
    .unwrap_err();
    assert_eq!(missing["code"], "not_found");
    manager.shutdown();
}
