//! Killing a session's whole process tree, not just the process we spawned.
//!
//! Windows: the agent is put in a Job Object right after it spawns, so everything it starts
//! joins the job; killing the job kills them all, and `KILL_ON_JOB_CLOSE` also kills them if
//! Foundry itself exits or crashes. Unix: portable-pty starts the agent as a session leader,
//! so its process group is the tree.

#[cfg(windows)]
mod imp {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    use crate::core::AppError;

    pub struct ProcessTree {
        job: HANDLE,
    }

    // SAFETY: a job handle is a kernel handle, usable from any thread.
    unsafe impl Send for ProcessTree {}
    unsafe impl Sync for ProcessTree {}

    fn os(what: &str, err: windows::core::Error) -> AppError {
        AppError::internal(format!("{what}: {}", err.message()))
    }

    impl ProcessTree {
        pub fn attach(child: &dyn portable_pty::Child) -> Result<Self, AppError> {
            let process = child
                .as_raw_handle()
                .ok_or_else(|| AppError::internal("spawned agent has no process handle"))?;
            // SAFETY: plain Win32 calls on handles we own; `info` outlives the call using it.
            unsafe {
                let job = CreateJobObjectW(None, PCWSTR::null())
                    .map_err(|e| os("could not create a job object", e))?;
                let tree = Self { job };
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&info).cast(),
                    u32::try_from(std::mem::size_of_val(&info)).unwrap_or(u32::MAX),
                )
                .map_err(|e| os("could not configure the job object", e))?;
                AssignProcessToJobObject(job, HANDLE(process))
                    .map_err(|e| os("could not add the agent to its job object", e))?;
                Ok(tree)
            }
        }

        pub fn kill(&self) -> Result<(), AppError> {
            // SAFETY: the job handle stays open until drop.
            unsafe { TerminateJobObject(self.job, 1) }
                .map_err(|e| os("could not kill the agent's processes", e))
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            // SAFETY: closed exactly once. Closing kills anything left (KILL_ON_JOB_CLOSE).
            unsafe {
                let _ = CloseHandle(self.job);
            }
        }
    }

    #[cfg(test)]
    pub fn pid_alive(pid: u32) -> bool {
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        const STILL_ACTIVE: u32 = 259;
        // SAFETY: query-only handle, closed before returning.
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return false;
            };
            let mut code = 0u32;
            let alive = GetExitCodeProcess(handle, &mut code).is_ok() && code == STILL_ACTIVE;
            let _ = CloseHandle(handle);
            alive
        }
    }
}

#[cfg(unix)]
mod imp {
    use std::process::Command;

    use crate::core::AppError;

    pub struct ProcessTree {
        group: u32,
    }

    impl ProcessTree {
        pub fn attach(child: &dyn portable_pty::Child) -> Result<Self, AppError> {
            let group = child
                .process_id()
                .ok_or_else(|| AppError::internal("spawned agent has no pid"))?;
            Ok(Self { group })
        }

        pub fn kill(&self) -> Result<(), AppError> {
            // The agent leads its own process group (portable-pty calls setsid).
            let status = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.group)])
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(AppError::internal(
                    "could not kill the agent's process group",
                ))
            }
        }
    }

    #[cfg(test)]
    pub fn pid_alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|s| s.success())
    }
}

#[cfg(test)]
pub use imp::pid_alive;
pub use imp::ProcessTree;

#[cfg(all(test, windows))]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::testing::fake_agent_path;

    fn gone_within(timeout: Duration, pids: &[u32]) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if pids.iter().all(|&pid| !pid_alive(pid)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// What protects against a Foundry crash: when the last handle to a session's job closes
    /// (the OS closes them all when a process dies), everything in the job is killed.
    /// Dropping the `ProcessTree` closes that handle, with no kill call made.
    #[test]
    fn closing_the_job_handle_kills_the_whole_tree() {
        // Sleep first so the child is spawned after the agent has joined the job.
        let mut agent = Command::new(fake_agent_path())
            .args([
                "--sleep-ms",
                "300",
                "--spawn-child",
                "60000",
                "--sleep-ms",
                "60000",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let tree = ProcessTree::attach(&agent).unwrap();
        let mut line = String::new();
        BufReader::new(agent.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let child: u32 = line
            .trim()
            .strip_prefix("child-pid=")
            .unwrap()
            .parse()
            .unwrap();
        let pids = [agent.id(), child];
        assert!(pids.iter().all(|&pid| pid_alive(pid)), "both running");
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            pids.iter().all(|&pid| pid_alive(pid)),
            "nothing kills them while the job is open"
        );

        drop(tree);

        assert!(
            gone_within(Duration::from_secs(3), &pids),
            "tree survived the handle closing"
        );
        // Reap it. (Windows reports exit code 0 for a kill-on-close, so the code proves nothing;
        // being gone in 3 s while set to sleep 60 s is the proof.)
        agent.wait().unwrap();
    }

    const HELPER: &str = "features::agent_sessions::tree::tests::crash_helper_holds_a_job";

    /// Not a test on its own: the body of the crash test's helper process. It puts a fake
    /// agent (and its child) in a job, prints their pids, then waits to be killed.
    #[test]
    #[ignore = "run only as the helper of crashing_the_owner_kills_the_whole_tree"]
    #[allow(
        clippy::zombie_processes,
        reason = "the helper is killed while its agent runs"
    )]
    fn crash_helper_holds_a_job() {
        if std::env::var_os("FOUNDRY_CRASH_HELPER").is_none() {
            return;
        }
        let mut agent = Command::new(fake_agent_path())
            .args([
                "--sleep-ms",
                "300",
                "--spawn-child",
                "60000",
                "--sleep-ms",
                "60000",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let _tree = ProcessTree::attach(&agent).unwrap();
        let mut line = String::new();
        BufReader::new(agent.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        println!(
            "pids={},{}",
            agent.id(),
            line.trim().strip_prefix("child-pid=").unwrap()
        );
        std::thread::sleep(Duration::from_secs(60));
    }

    /// A real crash: the process owning the job is hard-killed (no destructors, no cleanup
    /// code), and the OS closing its handles takes the whole tree down.
    #[test]
    fn crashing_the_owner_kills_the_whole_tree() {
        let mut helper = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                HELPER,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FOUNDRY_CRASH_HELPER", "1")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pids: Vec<u32> = BufReader::new(helper.stdout.take().unwrap())
            .lines()
            .map(Result::unwrap)
            // libtest prints `test <name> ... ` without a newline first, so look inside the line.
            .find_map(|line| {
                line.split_once("pids=")
                    .map(|(_, pids)| pids.trim().to_string())
            })
            .expect("helper printed the pids")
            .split(',')
            .map(|pid| pid.parse().unwrap())
            .collect();
        assert_eq!(pids.len(), 2);
        assert!(
            pids.iter().all(|&pid| pid_alive(pid)),
            "both running under the helper"
        );

        helper.kill().unwrap(); // TerminateProcess: as abrupt as a crash
        helper.wait().unwrap();

        assert!(
            gone_within(Duration::from_secs(3), &pids),
            "tree outlived its owner"
        );
    }
}
