use std::fs;

use tempfile::TempDir;

use super::*;
use crate::core::sessions::SessionKey;
use crate::core::setup;
use crate::testing::TestRepo;

const P: &str = "p1";

/// A committer for one repo, with a git that ignores the user's own global and system config
/// (the repo's local config still applies).
struct Env {
    repo: TestRepo,
    sessions: Arc<SessionRegistry>,
    committer: SetupCommitter,
    _git_home: TempDir,
}

impl Env {
    fn new(repo: TestRepo) -> Self {
        Self::with(repo, |git| git)
    }

    fn with(repo: TestRepo, git: impl FnOnce(Git) -> Git) -> Self {
        let git_home = tempfile::tempdir().unwrap();
        fs::write(git_home.path().join("gitconfig"), "").unwrap();
        let isolated = Git::system()
            .env("GIT_CONFIG_GLOBAL", git_home.path().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let sessions = Arc::new(SessionRegistry::default());
        let committer = SetupCommitter::new(repos, Arc::clone(&sessions), git(isolated));
        Self {
            repo,
            sessions,
            committer,
            _git_home: git_home,
        }
    }

    fn commit(&self) -> Result<CommitSetupResult, AppError> {
        self.committer.commit_setup(P)
    }

    fn commits(&self) -> u32 {
        self.repo
            .git(&["rev-list", "--count", "HEAD"])
            .parse()
            .unwrap()
    }

    /// `git status --porcelain` lines, untrimmed (`TestRepo::git` trims a leading " M").
    fn status(&self) -> String {
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain=v1", "--untracked-files=all"])
            .current_dir(self.repo.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    }
}

/// A git repo with one commit holding a source file.
fn repo_with_history() -> TestRepo {
    let repo = TestRepo::with_git();
    repo.write("src/main.rs", "fn main() {}\n");
    repo.write("src/lib.rs", "pub fn a() {}\n");
    repo.write("README.md", "# app\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "--quiet", "-m", "initial"]);
    repo
}

const SETUP_FILES: &[&str] = &[
    ".foundry/plan.json",
    ".foundry/project.json",
    ".foundry/prompts/s1.md",
    "AGENTS.md",
    "agents/roles.json",
    "docs/bakery/a.png",
    "docs/brain-dump.md",
    "vsm_data.js",
    "vsm_viewer.html",
];

fn write_setup(repo: &TestRepo) {
    for file in SETUP_FILES {
        repo.write(file, "x\n");
    }
}

fn committed_files(repo: &TestRepo) -> Vec<String> {
    let mut files: Vec<String> = repo
        .git(&["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .map(str::to_string)
        .collect();
    files.sort();
    files
}

#[test]
fn commits_only_the_listed_setup_paths() {
    let env = Env::new(repo_with_history());
    write_setup(&env.repo);
    // Foundry's own, but not setup: never committed.
    env.repo.write(".foundry/roadmap.json", "{}");
    env.repo.write(".foundry/runs/1/log.jsonl", "{}\n");
    env.repo.write(".foundry/reports/s1.json", "{}");
    env.repo.write("notes.txt", "mine");
    let before = env.repo.git(&["rev-parse", "HEAD"]);

    let result = env.commit().unwrap();

    assert_eq!(result.files, SETUP_FILES);
    assert_eq!(committed_files(&env.repo), SETUP_FILES);
    assert_eq!(result.commit, env.repo.git(&["rev-parse", "HEAD"]));
    assert_eq!(env.repo.git(&["log", "-1", "--format=%s"]), COMMIT_MESSAGE);
    assert_eq!(
        env.repo.git(&["rev-parse", "HEAD^"]),
        before,
        "a new commit"
    );
    let mut left: Vec<String> = env.status().lines().map(str::to_string).collect();
    left.sort();
    assert_eq!(
        left,
        [
            "?? .foundry/reports/s1.json",
            "?? .foundry/roadmap.json",
            "?? .foundry/runs/1/log.jsonl",
            "?? notes.txt",
        ]
    );

    // Modified (not only new) setup files are committed too.
    env.repo.write("AGENTS.md", "# edited\n");
    env.repo.write(".foundry/plan.json", "{\"revision\":2}");
    let result = env.commit().unwrap();
    assert_eq!(result.files, [".foundry/plan.json", "AGENTS.md"]);
    assert_eq!(committed_files(&env.repo), result.files);
}

#[test]
fn leaves_unrelated_modified_and_staged_files_untouched() {
    let env = Env::new(repo_with_history());
    env.repo.write("docs/old.md", "old\n");
    env.repo.git(&["add", "docs/old.md"]);
    env.repo.git(&["commit", "--quiet", "-m", "old doc"]);

    // Unstaged edit, staged edit, staged new file, and a setup deletion.
    env.repo.write("src/main.rs", "fn main() { edited() }\n");
    env.repo.write("src/lib.rs", "pub fn b() {}\n");
    env.repo.git(&["add", "src/lib.rs"]);
    env.repo.write("staged.txt", "staged\n");
    env.repo.git(&["add", "staged.txt"]);
    fs::remove_file(env.repo.path().join("docs/old.md")).unwrap();
    // The work tree then moves on past the staged version.
    env.repo.write("src/lib.rs", "pub fn c() {}\n");
    write_setup(&env.repo);

    let staged = env
        .repo
        .git(&["diff", "--cached", "--", "src", "staged.txt"]);
    let unstaged = env.repo.git(&["diff", "--", "src"]);

    let result = env.commit().unwrap();

    assert_eq!(result.files, SETUP_FILES);
    assert_eq!(committed_files(&env.repo), SETUP_FILES);
    let mut left: Vec<String> = env.status().lines().map(str::to_string).collect();
    left.sort();
    assert_eq!(
        left,
        [
            " D docs/old.md",
            " M src/main.rs",
            "A  staged.txt",
            "MM src/lib.rs"
        ],
        "deletions are not 'new or modified', so they stay"
    );
    assert_eq!(
        env.repo
            .git(&["diff", "--cached", "--", "src", "staged.txt"]),
        staged
    );
    assert_eq!(env.repo.git(&["diff", "--", "src"]), unstaged);
}

#[test]
fn nothing_to_commit_returns_head_and_no_files() {
    let env = Env::new(repo_with_history());
    let head = env.repo.git(&["rev-parse", "HEAD"]);
    let result = env.commit().unwrap();
    assert_eq!(
        result,
        CommitSetupResult {
            commit: head.clone(),
            files: vec![]
        }
    );
    assert_eq!(env.commits(), 1);

    write_setup(&env.repo);
    let first = env.commit().unwrap();
    let again = env.commit().unwrap();
    assert_eq!(again.commit, first.commit);
    assert!(again.files.is_empty());
    assert_eq!(env.commits(), 2);

    // A repo with no commits yet: no HEAD to report.
    let empty = Env::new(TestRepo::with_git());
    assert_eq!(empty.commit().unwrap().commit, "");
}

#[test]
fn no_git_identity_is_refused_and_says_how_to_set_it() {
    let repo = TestRepo::new();
    repo.git(&["init", "--quiet"]);
    let env = Env::new(repo);
    write_setup(&env.repo);

    let err = env.commit().unwrap_err();

    assert_eq!(err.code, ErrorCode::GitIdentityMissing);
    for text in [
        "user.name and user.email",
        "git config --global user.name",
        "git config --global user.email",
    ] {
        assert!(err.message.contains(text), "{text}: {}", err.message);
    }
    assert!(
        env.status().lines().all(|line| line.starts_with("??")),
        "nothing staged"
    );
    assert!(
        !fs::read_to_string(env.repo.path().join(".git/config"))
            .unwrap()
            .contains("[user]"),
        "the identity is never set for the user"
    );

    // Only email missing.
    env.repo.git(&["config", "user.name", "Someone"]);
    let err = env.commit().unwrap_err();
    assert_eq!(err.code, ErrorCode::GitIdentityMissing);
    assert!(
        err.message.contains("(user.email not set)"),
        "{}",
        err.message
    );
}

#[test]
fn not_a_repo_unknown_project_and_missing_git_have_their_own_codes() {
    let env = Env::new(TestRepo::new());
    write_setup(&env.repo);
    let err = env.commit().unwrap_err();
    assert_eq!(err.code, ErrorCode::NotAGitRepo);
    assert!(!env.repo.path().join(".git").exists(), "no git init");

    let err = env.committer.commit_setup("nope").unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);

    let env = Env::with(repo_with_history(), |_| {
        Git::new("foundry-definitely-not-git")
    });
    assert_eq!(env.commit().unwrap_err().code, ErrorCode::GitNotFound);
}

#[test]
fn a_failed_commit_reports_gits_message() {
    let env = Env::new(repo_with_history());
    env.repo.write(
        ".git/hooks/pre-commit",
        "#!/bin/sh\necho 'hook says no' >&2\nexit 1\n",
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let hook = env.repo.path().join(".git/hooks/pre-commit");
        fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_setup(&env.repo);

    let err = env.commit().unwrap_err();

    assert_eq!(err.code, ErrorCode::CommitFailed);
    assert!(err.message.contains("hook says no"), "{}", err.message);
    assert_eq!(env.commits(), 1);
}

#[test]
fn never_pushes_branches_or_amends() {
    let remote = tempfile::tempdir().unwrap();
    let env = Env::new(repo_with_history());
    let remote_path = remote.path().to_string_lossy().into_owned();
    env.repo.git(&["init", "--quiet", "--bare", &remote_path]);
    env.repo.git(&["remote", "add", "origin", &remote_path]);
    let branch = env.repo.git(&["symbolic-ref", "--short", "HEAD"]);
    env.repo
        .git(&["push", "--quiet", "--set-upstream", "origin", &branch]);
    let pushed = env.repo.git(&["rev-parse", "HEAD"]);
    let remote_head = || {
        env.repo
            .git(&["--git-dir", &remote_path, "rev-parse", &branch])
    };
    write_setup(&env.repo);

    let result = env.commit().unwrap();

    assert_eq!(remote_head(), pushed, "the remote is untouched");
    assert_eq!(env.repo.git(&["rev-parse", "HEAD^"]), pushed, "not amended");
    assert_eq!(env.repo.git(&["symbolic-ref", "--short", "HEAD"]), branch);
    assert_eq!(
        env.repo.git(&["branch", "--format=%(refname:short)"]),
        branch,
        "no new branch"
    );
    assert_ne!(result.commit, pushed);
}

#[test]
fn refused_while_an_agent_session_is_live_or_launching() {
    let env = Env::new(repo_with_history());
    write_setup(&env.repo);
    let key = SessionKey::new(P, "backend");
    env.sessions.reserve(&key, 4).unwrap();

    let err = env.commit().unwrap_err();
    assert_eq!(err.code, ErrorCode::SessionRunning);
    assert_eq!(env.commits(), 1);

    env.sessions.release(&key);
    assert_eq!(env.commit().unwrap().files, SETUP_FILES);
}

#[test]
fn covers_every_path_the_start_gate_checks() {
    for path in setup::SETUP_PATHS.iter().chain(setup::REWRITTEN_SETUP) {
        assert!(COMMIT_PATHS.contains(path), "{path}");
    }
    let env = Env::new(repo_with_history());
    write_setup(&env.repo);
    assert!(!setup::uncommitted(&Git::system(), env.repo.path()).is_empty());
    env.commit().unwrap();
    assert_eq!(
        setup::uncommitted(&Git::system(), env.repo.path()),
        Vec::<String>::new()
    );
}

#[test]
fn porcelain_states() {
    for state in ["??", " M", "M ", "MM", "A ", "AM", " T", "T "] {
        assert!(is_new_or_modified(state), "{state}");
    }
    for state in [" D", "D ", "AD", "MD", "UU", "AA", "DD", "AU", "UD", "!!"] {
        assert!(!is_new_or_modified(state), "{state}");
    }
}

#[test]
fn setup_status_lists_what_commit_setup_would_commit_and_is_empty_after() {
    let env = Env::new(repo_with_history());
    write_setup(&env.repo);
    env.repo.write(".foundry/reports/s1.json", "{}");
    env.repo.write("notes.txt", "mine");
    let head = env.repo.git(&["rev-parse", "HEAD"]);
    let before_status = env.status();

    let before = env.committer.setup_status(P).unwrap();
    assert!(before.needs_commit);
    assert_eq!(before.files, SETUP_FILES);
    // Read-only: nothing staged, nothing committed.
    assert_eq!(env.status(), before_status);
    assert_eq!(env.repo.git(&["rev-parse", "HEAD"]), head);

    assert_eq!(env.commit().unwrap().files, before.files);

    let after = env.committer.setup_status(P).unwrap();
    assert!(!after.needs_commit);
    assert!(after.files.is_empty());

    // An edit to a committed setup file counts again.
    env.repo.write("AGENTS.md", "changed\n");
    assert_eq!(env.committer.setup_status(P).unwrap().files, ["AGENTS.md"]);
}

#[test]
fn setup_status_outside_a_git_repo_is_not_a_git_repo() {
    let env = Env::new(TestRepo::new());
    assert_eq!(
        env.committer.setup_status(P).unwrap_err().code,
        ErrorCode::NotAGitRepo
    );
}
