//! Stand-in for a real agent CLI in process tests. Cross-platform (no shell scripts).
//!
//! Actions run in the order given, then the process exits:
//!   --echo                   copy stdin to stdout until EOF
//!   --echo-upper             for each stdin line, print `got:<LINE IN UPPERCASE>` until EOF
//!                            (proves input reached this process, not just a terminal echo)
//!   --burst <N>              print N lines `burst-00000`…, flushing after each
//!   --print-cwd              print `cwd=<current directory>`
//!   --print-env              print every environment variable as KEY=VALUE, sorted
//!   --sleep-ms <N>           sleep N milliseconds
//!   --spawn-child <N>        spawn a child fake-agent that sleeps N ms (sharing our stdio),
//!                            print `child-pid=<pid>` and carry on without waiting for it
//!   --exit <CODE>            exit code to use at the end (default 0)
//!
//! Unknown or malformed arguments exit with code 2.

use std::io::{self, BufRead, Write};
use std::process::{Command, ExitCode};
use std::time::Duration;

enum Action {
    Echo,
    EchoUpper,
    Burst(u64),
    PrintCwd,
    PrintEnv,
    Sleep(u64),
    SpawnChild(u64),
}

fn main() -> ExitCode {
    match parse(std::env::args().skip(1)).and_then(|(actions, code)| run(actions).map(|()| code)) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("fake-agent: {err}");
            ExitCode::from(2)
        }
    }
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<(Vec<Action>, u8), String> {
    let mut actions = Vec::new();
    let mut code = 0;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--echo" => actions.push(Action::Echo),
            "--echo-upper" => actions.push(Action::EchoUpper),
            "--burst" => actions.push(Action::Burst(number(&value("--burst")?)?)),
            "--print-cwd" => actions.push(Action::PrintCwd),
            "--print-env" => actions.push(Action::PrintEnv),
            "--sleep-ms" => actions.push(Action::Sleep(number(&value("--sleep-ms")?)?)),
            "--spawn-child" => actions.push(Action::SpawnChild(number(&value("--spawn-child")?)?)),
            "--exit" => {
                code = value("--exit")?
                    .parse()
                    .map_err(|_| "--exit needs a code from 0 to 255".to_string())?
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok((actions, code))
}

fn number(text: &str) -> Result<u64, String> {
    text.parse().map_err(|_| format!("not a number: {text}"))
}

fn run(actions: Vec<Action>) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    for action in actions {
        match action {
            Action::Echo => {
                io::copy(&mut io::stdin().lock(), &mut stdout).map_err(|e| e.to_string())?;
            }
            Action::EchoUpper => {
                for line in io::stdin().lock().lines() {
                    let line = line.map_err(|e| e.to_string())?;
                    writeln!(stdout, "got:{}", line.trim_end().to_uppercase())
                        .map_err(|e| e.to_string())?;
                    stdout.flush().map_err(|e| e.to_string())?;
                }
            }
            Action::Burst(lines) => {
                for i in 0..lines {
                    writeln!(stdout, "burst-{i:05}").map_err(|e| e.to_string())?;
                    stdout.flush().map_err(|e| e.to_string())?;
                }
            }
            Action::PrintCwd => {
                let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
                writeln!(stdout, "cwd={}", cwd.display()).map_err(|e| e.to_string())?;
            }
            Action::PrintEnv => {
                let mut vars: Vec<(String, String)> = std::env::vars().collect();
                vars.sort();
                for (key, value) in vars {
                    writeln!(stdout, "{key}={value}").map_err(|e| e.to_string())?;
                }
            }
            Action::Sleep(ms) => std::thread::sleep(Duration::from_millis(ms)),
            Action::SpawnChild(ms) => {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let child = Command::new(exe)
                    .args(["--sleep-ms", &ms.to_string()])
                    .spawn()
                    .map_err(|e| e.to_string())?;
                writeln!(stdout, "child-pid={}", child.id()).map_err(|e| e.to_string())?;
            }
        }
        stdout.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}
