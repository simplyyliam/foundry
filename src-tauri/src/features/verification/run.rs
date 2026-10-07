//! Running one confirmed test command through the platform shell, in the repo root, with the
//! subscription environment, a time limit and a cancel flag. Timeout and cancel kill the
//! command's whole process tree (`core::process_tree`).

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::core::process_tree::ProcessTree;
use crate::core::subscription::subscription_env;
use crate::core::{AppError, ErrorCode};

/// How much of the output (stdout and stderr together) is kept.
pub const TAIL_BYTES: usize = 8 * 1024;
const POLL: Duration = Duration::from_millis(50);

pub struct Outcome {
    /// `None` when it was killed (timeout) or couldn't start.
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub tail: String,
    pub timed_out: bool,
}

impl Outcome {
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }
}

/// `cmd.exe /d /s /c "<command>"` on Windows, `sh -c <command>` elsewhere.
fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = Command::new("cmd.exe");
        // `/s` strips exactly the outer quotes, so the command line reaches cmd as written.
        cmd.args(["/d", "/s", "/c"])
            .raw_arg(format!("\"{command}\""))
            .creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = Command::new("sh");
        // Its own process group, which is the tree `ProcessTree` kills.
        cmd.arg("-c").arg(command).process_group(0);
        cmd
    }
}

/// Keeps the last [`TAIL_BYTES`] of everything written to it.
#[derive(Clone, Default)]
struct Tail(Arc<Mutex<Vec<u8>>>);

impl Tail {
    fn push(&self, bytes: &[u8]) {
        let mut buf = self.0.lock().unwrap_or_else(|p| p.into_inner());
        buf.extend_from_slice(bytes);
        if buf.len() > 2 * TAIL_BYTES {
            let cut = buf.len() - TAIL_BYTES;
            buf.drain(..cut);
        }
    }

    fn text(&self) -> String {
        let buf = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let mut start = buf.len().saturating_sub(TAIL_BYTES);
        // Don't start halfway through a UTF-8 character.
        while start < buf.len() && start > 0 && (buf[start] & 0b1100_0000) == 0b1000_0000 {
            start += 1;
        }
        String::from_utf8_lossy(&buf[start..]).into_owned()
    }
}

fn drain(pipe: Option<impl Read + Send + 'static>, tail: Tail) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let Some(mut pipe) = pipe else { return };
        let mut chunk = [0u8; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            tail.push(&chunk[..n]);
        }
    })
}

/// Runs `command` in `cwd`. A command that can't start is a failed run (its tail says why),
/// not an error. `cancelled` when `cancel` is set while it runs.
pub fn run(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<Outcome, AppError> {
    let started = Instant::now();
    let mut cmd = shell(command);
    cmd.current_dir(cwd)
        .env_clear()
        .envs(subscription_env())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            return Ok(Outcome {
                exit_code: None,
                duration: started.elapsed(),
                tail: format!("Foundry couldn't start the shell: {err}"),
                timed_out: false,
            })
        }
    };
    let tree = match ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
    };
    let tail = Tail::default();
    let readers = [
        drain(child.stdout.take(), tail.clone()),
        drain(child.stderr.take(), tail.clone()),
    ];

    let deadline = started + timeout;
    let ended = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(Some(status.code())),
            Ok(None) => {}
            Err(err) => break Err(AppError::from(err)),
        }
        if cancel.load(Ordering::SeqCst) {
            break Err(AppError::new(
                ErrorCode::Cancelled,
                "verification cancelled",
            ));
        }
        if Instant::now() >= deadline {
            break Ok(None);
        }
        thread::sleep(POLL);
    };
    // Always: on timeout or cancel it stops the command, and after a normal exit it stops
    // anything the command left running in the background (which would hold the pipes open).
    let _ = tree.kill();
    let _ = child.wait();
    drop(tree);
    for reader in readers {
        let _ = reader.join();
    }
    let duration = started.elapsed();
    let exit = ended?;
    Ok(Outcome {
        exit_code: exit.flatten(),
        duration,
        tail: tail.text(),
        timed_out: exit.is_none(),
    })
}
