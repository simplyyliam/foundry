//! Black-box tests for the fake-agent binary.

#[path = "../src/testing.rs"]
mod testing;

use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use testing::fake_agent_path;

fn agent() -> Command {
    Command::new(fake_agent_path())
}

fn run(args: &[&str]) -> Output {
    agent().args(args).stdin(Stdio::null()).output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn is_alive(pid: u32) -> bool {
    if cfg!(windows) {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
    } else {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    }
}

#[test]
fn no_args_exits_zero_silently() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

#[test]
fn echoes_stdin() {
    let mut child = agent()
        .arg("--echo")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = "hello\nsecond line ✓\n";
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(stdout(&out), input);
}

#[test]
fn prints_environment_variables() {
    let out = agent()
        .arg("--print-env")
        .env("FOUNDRY_TEST_VAR", "some value=with equals")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text
        .lines()
        .any(|line| line == "FOUNDRY_TEST_VAR=some value=with equals"));
}

#[test]
fn print_env_reflects_removed_vars() {
    let out = agent()
        .arg("--print-env")
        .env_clear()
        .env("ONLY_ME", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = stdout(&out);
    assert!(text.lines().any(|line| line == "ONLY_ME=1"));
    assert!(!text.contains("FOUNDRY_TEST_VAR"));
}

#[test]
fn sleeps_on_request() {
    let start = Instant::now();
    let out = run(&["--sleep-ms", "300"]);
    assert!(out.status.success());
    assert!(start.elapsed() >= Duration::from_millis(300));
}

#[test]
fn exits_with_chosen_code() {
    for code in [0, 1, 3, 42, 255] {
        let out = run(&["--exit", &code.to_string()]);
        assert_eq!(out.status.code(), Some(code));
    }
}

#[test]
fn rejects_bad_arguments_with_code_2() {
    for args in [
        &["--bogus"][..],
        &["--sleep-ms"],
        &["--sleep-ms", "x"],
        &["--exit", "256"],
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(!out.stderr.is_empty());
    }
}

#[test]
fn actions_run_in_order_then_exit_code_applies() {
    let mut child = agent()
        .args(["--echo", "--print-env", "--exit", "7"])
        .env("ORDER_MARK", "after-echo")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"first\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(7));
    let text = stdout(&out);
    assert!(text.starts_with("first\n"));
    assert!(text.contains("ORDER_MARK=after-echo"));
}

#[test]
fn spawns_a_sleeping_child_that_outlives_it() {
    let start = Instant::now();
    let mut parent = agent()
        .args(["--spawn-child", "3000", "--exit", "5"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = parent.stdout.take().unwrap();

    // The parent exits right away with its own code...
    let status = parent.wait().unwrap();
    assert_eq!(status.code(), Some(5));

    // ...and reports the child's pid; read just that line (the child keeps the pipe open).
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while out.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
        line.push(byte[0]);
    }
    let line = String::from_utf8(line).unwrap();
    let pid: u32 = line
        .trim()
        .strip_prefix("child-pid=")
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(pid, parent.id());
    assert!(is_alive(pid), "child {pid} should still be sleeping");

    // EOF arrives only once the child exits and drops the inherited stdout.
    let mut rest = String::new();
    out.read_to_string(&mut rest).unwrap();
    assert!(start.elapsed() >= Duration::from_millis(3000));
    // Pipe close and process exit are not simultaneous; give the OS a moment.
    let deadline = Instant::now() + Duration::from_secs(5);
    while is_alive(pid) {
        assert!(Instant::now() < deadline, "child {pid} should have exited");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn echo_upper_answers_each_line() {
    let mut child = agent()
        .arg("--echo-upper")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"hello\nworld\r\n")
        .unwrap();
    let mut out = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut out)
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(out.replace("\r\n", "\n"), "got:HELLO\ngot:WORLD\n");
}

#[test]
fn burst_block_prints_the_same_lines_as_burst() {
    let block = run(&["--burst-block", "3"]);
    let lines = run(&["--burst", "3"]);
    assert!(block.status.success());
    assert_eq!(stdout(&block), stdout(&lines));
}

#[test]
fn bursts_numbered_lines_and_prints_cwd() {
    let out = run(&["--burst", "3", "--print-cwd"]);
    assert!(out.status.success());
    let text = stdout(&out).replace("\r\n", "\n");
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        text,
        format!(
            "burst-00000\nburst-00001\nburst-00002\ncwd={}\n",
            cwd.display()
        )
    );
}
