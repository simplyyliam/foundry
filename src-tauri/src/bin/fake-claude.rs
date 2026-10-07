//! Stand-in for the `claude` CLI in process tests. Tests copy it to their own folder as
//! `claude(.exe)`, so files next to it configure it without touching the environment.
//!
//!   --version                 print `version.txt` from this exe's folder, else
//!                             `9.9.9 (Fake Claude)`
//!   auth status [--json]      print `auth.json` from this exe's folder (default: signed in
//!                             with a subscription); exit 1 when it says `loggedIn: false`
//!   -p <flags…>               read the prompt from stdin, reply like `--output-format json`.
//!                             Requires the flags Foundry uses (then optionally
//!                             `--model <name>`). The prompt picks the reply:
//!                               FAKE:ENV    the environment, one KEY=VALUE per line
//!                               FAKE:ARGS   the arguments, one per line
//!                               FAKE:CWD    the working directory
//!                               FAKE:ERROR  an `is_error: true` reply
//!                               FAKE:CRASH  nothing on stdout, a message on stderr, exit 3
//!                               FAKE:SLEEP  spawn a sleeping child, write both pids to
//!                                           `pids.txt` next to this exe, then sleep 60 s
//!                               otherwise   `reply.txt` from this exe's folder, else "ok"
//!   --sleep-child             sleep 60 s (the FAKE:SLEEP child)
//!   [--model <m>] <message>   interactive mode, like `claude "<message>"`. The message
//!                             containing:
//!                               FAKE:ENV    write the environment to `env.json` next to
//!                                           this exe (exact; terminal output isn't), exit
//!                               FAKE:ARGS   write the arguments to `args.json`, exit
//!                               otherwise   print `ready`, then sleep 60 s

use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

const FLAGS: &[&str] = &[
    "-p",
    "--output-format",
    "json",
    "--tools",
    "",
    "--strict-mcp-config",
    "--no-session-persistence",
];

fn here() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_default()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            // `version.txt` next to the exe (a test's own copy) overrides the version line.
            match std::fs::read_to_string(here().join("version.txt")) {
                Ok(text) => print!("{text}"),
                Err(_) => println!("9.9.9 (Fake Claude)"),
            }
            ExitCode::SUCCESS
        }
        Some("auth") => {
            let auth = std::fs::read_to_string(here().join("auth.json")).unwrap_or_else(|_| {
                r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"someone@example.invalid","subscriptionType":"pro"}"#.into()
            });
            println!("{auth}");
            if auth.replace(' ', "").contains("\"loggedIn\":false") {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Some("--sleep-child") => {
            std::thread::sleep(Duration::from_secs(60));
            ExitCode::SUCCESS
        }
        Some("-p") => print_mode(&args),
        Some(_) => interactive(&args),
        None => {
            eprintln!("fake-claude: unexpected arguments {args:?}");
            ExitCode::from(2)
        }
    }
}

fn print_mode(args: &[String]) -> ExitCode {
    let model_ok = match &args[FLAGS.len().min(args.len())..] {
        [] => true,
        [flag, model] => flag == "--model" && !model.is_empty(),
        _ => false,
    };
    if !args.starts_with(&FLAGS.iter().map(|f| f.to_string()).collect::<Vec<_>>()) || !model_ok {
        eprintln!("fake-claude: expected {FLAGS:?}, got {args:?}");
        return ExitCode::from(2);
    }
    let mut prompt = String::new();
    if io::stdin().read_to_string(&mut prompt).is_err() {
        return ExitCode::from(2);
    }
    if prompt.contains("FAKE:CRASH") {
        eprintln!("fake-claude: something broke");
        return ExitCode::from(3);
    }
    if prompt.contains("FAKE:ERROR") {
        let reply = serde_json::json!({
            "type": "result",
            "subtype": "error_during_execution",
            "is_error": true,
            "result": "usage limit reached",
        });
        println!("{reply}");
        return ExitCode::from(1);
    }
    let result = if prompt.contains("FAKE:CWD") {
        std::env::current_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_default()
    } else if prompt.contains("FAKE:ENV") {
        let mut vars: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
        vars.sort();
        vars.join("\n")
    } else if prompt.contains("FAKE:ARGS") {
        args.join("\n")
    } else if prompt.contains("FAKE:SLEEP") {
        let child = Command::new(std::env::current_exe().unwrap_or_default())
            .arg("--sleep-child")
            .spawn();
        let child_pid = child.map(|c| c.id()).unwrap_or(0);
        let _ = std::fs::write(
            here().join("pids.txt"),
            format!("{},{child_pid}", std::process::id()),
        );
        std::thread::sleep(Duration::from_secs(60));
        String::new()
    } else {
        std::fs::read_to_string(here().join("reply.txt")).unwrap_or_else(|_| "ok".into())
    };
    let reply = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": result,
        "session_id": "fake",
    });
    println!("{reply}");
    ExitCode::SUCCESS
}

fn interactive(args: &[String]) -> ExitCode {
    let message = match args {
        [message] => message,
        [flag, model, message] if flag == "--model" && !model.is_empty() => message,
        _ => {
            eprintln!("fake-claude: unexpected arguments {args:?}");
            return ExitCode::from(2);
        }
    };
    if message.contains("FAKE:ENV") {
        let vars: std::collections::BTreeMap<String, String> = std::env::vars().collect();
        let _ = std::fs::write(here().join("env.json"), serde_json::json!(vars).to_string());
        println!("done");
    } else if message.contains("FAKE:ARGS") {
        let _ = std::fs::write(
            here().join("args.json"),
            serde_json::json!(args).to_string(),
        );
        println!("done");
    } else {
        println!("ready");
        std::thread::sleep(Duration::from_secs(60));
    }
    ExitCode::SUCCESS
}
