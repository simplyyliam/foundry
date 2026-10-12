use std::fs;
use std::process::Command;

use serde_json::{json, Value};
use tempfile::TempDir;

use super::*;
use crate::core::report::DONE_OVERRIDE_PREFIX;
use crate::core::sessions::SessionKey;
use crate::testing::TestRepo;

const P: &str = "p1";
const S: &str = "s-login";

/// A committer for one repo, with a git that ignores the user's own global and system config
/// (the repo's local config still applies).
struct Env {
    repo: TestRepo,
    sessions: Arc<SessionRegistry>,
    committer: SliceCommitter,
    _git_home: TempDir,
}

impl Env {
    fn new(repo: TestRepo) -> Self {
        let git_home = tempfile::tempdir().unwrap();
        fs::write(git_home.path().join("gitconfig"), "").unwrap();
        let git = Git::system()
            .env("GIT_CONFIG_GLOBAL", git_home.path().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let sessions = Arc::new(SessionRegistry::default());
        let committer = SliceCommitter::new(repos, Arc::clone(&sessions), git);
        Self {
            repo,
            sessions,
            committer,
            _git_home: git_home,
        }
    }

    /// A git repo with a plan holding slice [`S`] and requirements r-1..r-3, all committed.
    fn with_plan() -> Self {
        let env = Self::new(TestRepo::with_git());
        env.plan("Login", "Users can log in with email. Then more.", &[]);
        env.repo.write("src/app.ts", "app\n");
        env.repo.write("src/old.ts", "old\n");
        env.repo.write(".gitignore", "node_modules/\n*.log\n");
        env.repo.git(&["add", "-A"]);
        env.repo.git(&["commit", "--quiet", "-m", "initial"]);
        env
    }

    fn plan(&self, name: &str, deliverable: &str, decisions: &[&str]) {
        let plan = json!({
            "version": 1,
            "revision": 1,
            "slices": [{
                "id": S, "name": name, "delivers": [deliverable], "dependsOn": [],
                "status": "in_progress", "decisions": decisions, "openQuestions": [],
                "startCommit": null
            }],
            "requirements": [
                { "id": "r-1", "text": "a", "kind": "behavior", "sliceId": S },
                { "id": "r-2", "text": "b", "kind": "behavior", "sliceId": S },
                { "id": "r-3", "text": "c", "kind": "behavior", "sliceId": S },
                { "id": "r-9", "text": "d", "kind": "constraint", "sliceId": null, "projectWide": true },
                { "id": "r-x", "text": "e", "kind": "behavior", "sliceId": "other" }
            ]
        });
        self.repo.write(".foundry/plan.json", &plan.to_string());
    }

    fn roles(&self) {
        let roles = json!({ "agents": [
            { "id": "ada", "displayName": "Ada Lovelace" },
            { "id": "bo", "displayName": "" }
        ]});
        self.repo.write("agents/roles.json", &roles.to_string());
    }

    fn report(&self, agent: &str) -> Value {
        let report = json!({ "sliceId": S, "agent": agent, "requirements": [],
            "testsRun": [], "blockers": [], "notes": "" });
        self.repo.write(
            &format!(".foundry/reports/{S}.{agent}.json"),
            &report.to_string(),
        );
        report
    }

    /// A stored verification of the reports `ada` and `bo` wrote, with r-1 and r-3 met.
    fn review(&self, passed: bool) {
        let reports: Vec<Value> = ["ada", "bo"]
            .iter()
            .map(|agent| {
                json!({ "path": format!(".foundry/reports/{S}.{agent}.json"),
                "agent": agent, "report": self.report(agent) })
            })
            .collect();
        let review = json!({
            "sliceId": S, "passed": passed, "findings": [], "testRuns": [],
            "reports": reports, "report": reports[0]["report"],
            "requirements": [
                { "id": "r-1", "status": "met", "reportedBy": [] },
                { "id": "r-2", "status": "not_met", "reportedBy": [] },
                { "id": "r-3", "status": "met", "reportedBy": [] }
            ],
            "verifiedAt": 1
        });
        self.repo
            .write(&format!(".foundry/reviews/{S}.json"), &review.to_string());
    }

    fn commit(&self, message: Option<&str>) -> Result<CommitSliceResult, AppError> {
        self.committer.commit_slice(P, S, message)
    }

    fn head(&self) -> String {
        self.repo.git(&["rev-parse", "HEAD"])
    }

    /// The message stored in HEAD, byte for byte.
    fn head_message(&self) -> String {
        let output = Command::new("git")
            .args(["cat-file", "commit", "HEAD"])
            .current_dir(self.repo.path())
            .output()
            .unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        text.split_once("\n\n").unwrap().1.to_string()
    }

    /// `git status --porcelain` lines, untrimmed.
    fn status(&self) -> String {
        let output = Command::new("git")
            .args(["status", "--porcelain=v1", "--untracked-files=all"])
            .current_dir(self.repo.path())
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    }
}

fn file(path: &str, change: FileChange) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        change,
        foundry_owned: is_foundry_owned(path),
    }
}

fn changes(env: &Env) -> Vec<(String, FileChange, bool)> {
    env.committer
        .commit_status(P, None)
        .unwrap()
        .files
        .into_iter()
        .map(|f| (f.path, f.change, f.foundry_owned))
        .collect()
}

// ---------- commit_status ----------

#[test]
fn commit_status_classifies_changes_and_marks_foundry_owned_files() {
    let env = Env::with_plan();
    env.repo.write("src/app.ts", "changed\n");
    env.repo.write("src/new.ts", "new\n");
    fs::remove_file(env.repo.path().join("src/old.ts")).unwrap();
    env.repo.write("README.md", "staged\n");
    env.repo.git(&["add", "README.md"]);
    env.repo.git(&["mv", ".gitignore", ".gitignore2"]);
    env.repo.write("AGENTS.md", "x");
    env.report("ada");
    env.repo.write(".foundry/reviews/s-login.json", "{}");
    env.repo.write(".foundry/reviews/digest.md", "x");
    env.repo.write(".foundry/reviews/notes/a.json", "{}");
    env.repo.write(".foundry/roadmap.json", "{}");
    let before = env.status();

    let status = env.committer.commit_status(P, None).unwrap();

    use FileChange::*;
    let got: Vec<(&str, FileChange, bool)> = status
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.change, f.foundry_owned))
        .collect();
    assert_eq!(
        got,
        [
            (".foundry/reports/s-login.ada.json", New, true),
            (".foundry/reviews/digest.md", New, false),
            (".foundry/reviews/notes/a.json", New, false),
            (".foundry/reviews/s-login.json", New, true),
            (".foundry/roadmap.json", New, false),
            (".gitignore2", Renamed, false),
            ("AGENTS.md", New, true),
            ("README.md", New, false),
            ("src/app.ts", Modified, false),
            ("src/new.ts", New, false),
            ("src/old.ts", Deleted, false),
        ]
    );
    assert_eq!(status.total, 11);
    assert!(!status.truncated);
    assert_eq!(status.head, env.head());
    assert!(!status.branch.is_empty());
    assert_eq!(status.commit_type, None, "no slice asked about");
    assert_eq!(env.status(), before, "read-only");
}

#[test]
fn ignored_files_never_appear_and_are_never_committed() {
    let env = Env::with_plan();
    env.repo.write("node_modules/x/index.js", "x");
    env.repo.write("debug.log", "x");
    env.repo.write("src/app.ts", "changed\n");

    assert_eq!(
        changes(&env),
        [("src/app.ts".to_string(), FileChange::Modified, false)]
    );
    let result = env.commit(None).unwrap();
    assert_eq!(result.files, ["src/app.ts"]);
    assert!(env
        .repo
        .git(&["ls-files", "node_modules", "debug.log"])
        .is_empty());
}

#[test]
fn commit_status_is_cut_at_500_files_and_says_so() {
    let env = Env::with_plan();
    for i in 0..502 {
        env.repo.write(&format!("gen/f{i:03}.txt"), "x");
    }
    let status = env.committer.commit_status(P, None).unwrap();
    assert_eq!(status.files.len(), MAX_STATUS_FILES);
    assert_eq!(status.total, 502);
    assert!(status.truncated);
    assert_eq!(status.files[0].path, "gen/f000.txt");
}

#[test]
fn commit_status_with_a_slice_gives_the_type_commit_slice_would_pick() {
    let env = Env::with_plan();
    env.repo.write("src/app.test.ts", "t");
    let status = env.committer.commit_status(P, Some(S)).unwrap();
    assert_eq!(status.commit_type, Some(CommitType::Test));
    assert_eq!(
        status.type_reason.as_deref(),
        Some("only test files changed")
    );

    let result = env.commit(None).unwrap();
    assert_eq!(result.commit_type, Some(CommitType::Test));
    assert_eq!(result.type_reason, status.type_reason);
    assert!(result.message.starts_with("test(login): "));

    let clean = env.committer.commit_status(P, Some(S)).unwrap();
    assert!(clean.files.is_empty());
    assert_eq!(clean.commit_type, None, "nothing to commit");
    assert_eq!(
        env.committer
            .commit_status(P, Some("nope"))
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

// ---------- commit_slice ----------

#[test]
fn commit_slice_commits_every_change_including_agent_work() {
    let env = Env::with_plan();
    let before = env.head();
    env.repo.write("src/app.ts", "changed\n");
    env.repo.write("src/new.ts", "new\n");
    fs::remove_file(env.repo.path().join("src/old.ts")).unwrap();
    env.repo.write("docs/notes.md", "n");
    env.report("ada");

    let result = env.commit(None).unwrap();

    assert_eq!(
        result.files,
        [
            ".foundry/reports/s-login.ada.json",
            "docs/notes.md",
            "src/app.ts",
            "src/new.ts",
            "src/old.ts"
        ]
    );
    assert_eq!(result.commit, env.head());
    assert_eq!(
        env.repo.git(&["rev-parse", "HEAD~1"]),
        before,
        "a new commit, not an amend"
    );
    assert!(env.status().is_empty(), "everything committed");
    assert_eq!(env.head_message(), result.message);
}

#[test]
fn a_callers_message_is_used_exactly() {
    let env = Env::with_plan();
    env.repo.write("src/app.ts", "changed\n");
    let message = "My own words\n\n# not a comment  \nTrailing: yes";

    let result = env.commit(Some(message)).unwrap();

    assert_eq!(result.message, message);
    assert_eq!(result.commit_type, None);
    assert_eq!(result.type_reason, None);
    assert_eq!(env.head_message().trim_end_matches('\n'), message);
    assert!(env.head_message().contains("# not a comment  \n"));
}

const LOGIN_SUBJECT: &str = "feat(login): Users can log in with email";

#[test]
fn the_generated_message_for_a_passed_verification() {
    let env = Env::with_plan();
    env.roles();
    env.review(true);
    env.repo.write("src/app.ts", "changed\n");

    let result = env.commit(None).unwrap();

    assert_eq!(
        result.message,
        format!(
            "{LOGIN_SUBJECT}\n\nRequirements met: 2 of 3 (r-1, r-3)\nVerification: passed\n\
             Agents: Ada Lovelace, bo\n"
        )
    );
    assert_eq!(env.head_message(), result.message);
    assert_eq!(result.commit_type, Some(CommitType::Feat));
}

#[test]
fn the_generated_message_for_an_overridden_verification_gives_the_reason() {
    let env = Env::with_plan();
    let decision =
        format!("{DONE_OVERRIDE_PREFIX}; it failed its last verification): checked by hand");
    env.plan(
        "Login",
        "Users can log in with email.",
        &["other", &decision],
    );
    env.review(false);
    env.repo.write("src/app.ts", "changed\n");

    let message = env.commit(None).unwrap().message;
    assert!(
        message.contains("\nVerification: overridden (checked by hand)\n"),
        "{message}"
    );
    assert!(message.contains("Requirements met: 2 of 3 (r-1, r-3)"));
}

#[test]
fn the_generated_message_for_a_failed_verification() {
    let env = Env::with_plan();
    env.review(false);
    env.repo.write("src/app.ts", "changed\n");
    let message = env.commit(None).unwrap().message;
    assert!(message.contains("\nVerification: failed\n"), "{message}");
    assert!(
        message.contains("\nAgents: ada, bo\n"),
        "no roles.json: ids"
    );
}

#[test]
fn the_generated_message_without_a_verification_says_not_run() {
    let env = Env::with_plan();
    env.repo.write("src/app.ts", "changed\n");
    let message = env.commit(None).unwrap().message;
    assert_eq!(
        message,
        format!(
            "{LOGIN_SUBJECT}\n\nRequirements met: 0 of 3\nVerification: not run\nAgents: none\n"
        )
    );
}

#[test]
fn a_report_changed_after_a_pass_reads_as_stale() {
    let env = Env::with_plan();
    env.review(true);
    env.repo.write(
        ".foundry/reports/s-login.ada.json",
        &json!({ "sliceId": S, "agent": "ada", "requirements": [], "testsRun": [],
            "blockers": ["new"], "notes": "" })
        .to_string(),
    );
    let message = env.commit(None).unwrap().message;
    assert!(message.contains("\nVerification: stale"), "{message}");
}

#[test]
fn a_long_first_line_is_cut_at_72_characters() {
    let env = Env::with_plan();
    env.plan(
        "Login",
        &"Users can log in with a very long email address ".repeat(3),
        &[],
    );
    env.repo.write("src/app.ts", "changed\n");
    let message = env.commit(None).unwrap().message;
    let subject = message.lines().next().unwrap();
    assert!(subject.chars().count() <= 72, "{subject}");
    assert!(subject.ends_with('…'), "{subject}");
    assert!(subject.starts_with("feat(login): Users can log in"));
}

#[test]
fn refused_while_a_session_is_live_or_launching() {
    let env = Env::with_plan();
    env.repo.write("src/app.ts", "changed\n");
    let before = env.head();
    let key = SessionKey::new(P, "backend");
    env.sessions.reserve(&key, 4).unwrap();

    let err = env.commit(None).unwrap_err();
    assert_eq!(err.code, ErrorCode::SessionRunning);
    assert_eq!(env.head(), before);
    assert!(env.status().starts_with(" M"), "nothing staged");

    env.sessions.release(&key);
    assert_eq!(env.commit(None).unwrap().files, ["src/app.ts"]);
}

#[test]
fn no_git_identity_is_refused_and_says_how_to_set_it() {
    let repo = TestRepo::new();
    repo.git(&["init", "--quiet"]);
    let env = Env::new(repo);
    env.plan("Login", "x", &[]);

    let err = env.commit(None).unwrap_err();

    assert_eq!(err.code, ErrorCode::GitIdentityMissing);
    assert!(
        err.message.contains("git config --global user.name"),
        "{}",
        err.message
    );
    assert!(
        err.message.contains("git config --global user.email"),
        "{}",
        err.message
    );
    assert!(
        env.status().lines().all(|l| l.starts_with("??")),
        "nothing staged"
    );
    assert!(!fs::read_to_string(env.repo.path().join(".git/config"))
        .unwrap()
        .contains("[user]"));
}

#[test]
fn nothing_to_commit_is_not_an_error() {
    let env = Env::with_plan();
    let before = env.head();
    let result = env.commit(None).unwrap();
    assert_eq!(result.commit, before);
    assert!(result.files.is_empty());
    assert!(result.message.is_empty());
    assert_eq!(env.head(), before);
}

#[test]
fn unknown_project_or_slice_and_not_a_git_repo_have_their_own_codes() {
    let env = Env::with_plan();
    assert_eq!(
        env.committer
            .commit_slice("nope", S, None)
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        env.committer
            .commit_slice(P, "nope", None)
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    let plain = Env::new(TestRepo::new());
    plain.plan("Login", "x", &[]);
    assert_eq!(plain.commit(None).unwrap_err().code, ErrorCode::NotAGitRepo);
    assert_eq!(
        plain.committer.commit_status(P, None).unwrap_err().code,
        ErrorCode::NotAGitRepo
    );
}

#[test]
fn a_failing_commit_is_commit_failed_with_gits_message() {
    let env = Env::with_plan();
    env.repo.write("src/app.ts", "changed\n");
    let hooks = env.repo.path().join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(
        hooks.join("pre-commit"),
        "#!/bin/sh\necho 'hook says no' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(hooks.join("pre-commit"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let err = env.commit(None).unwrap_err();
    assert_eq!(err.code, ErrorCode::CommitFailed);
    assert!(err.message.contains("hook says no"), "{}", err.message);
}

#[test]
fn never_pushes_branches_or_amends() {
    let env = Env::with_plan();
    let remote = TestRepo::new();
    remote.git(&["init", "--quiet", "--bare"]);
    let remote_path = remote.path().to_string_lossy().into_owned();
    env.repo.git(&["remote", "add", "origin", &remote_path]);
    let branch = env.repo.git(&["symbolic-ref", "--short", "HEAD"]);
    env.repo.git(&["push", "--quiet", "origin", &branch]);
    let pushed = remote.git(&["rev-parse", &branch]);
    let branches = env.repo.git(&["branch", "--list"]);
    let before = env.head();
    env.repo.write("src/app.ts", "changed\n");

    let result = env.commit(None).unwrap();

    assert_eq!(
        remote.git(&["rev-parse", &branch]),
        pushed,
        "the remote is untouched"
    );
    assert_eq!(
        env.repo.git(&["branch", "--list"]),
        branches,
        "no new branch"
    );
    assert_eq!(env.repo.git(&["symbolic-ref", "--short", "HEAD"]), branch);
    assert_eq!(
        env.repo.git(&["rev-parse", "HEAD~1"]),
        before,
        "not an amend"
    );
    assert_eq!(result.commit, env.head());
}

// ---------- commit type rules ----------

fn type_of(name: &str, deliverable: &str, files: &[ChangedFile]) -> (CommitType, String) {
    let c = commit_type(name, deliverable, files);
    (c.commit_type, c.reason)
}

#[test]
fn only_test_files_is_test() {
    use FileChange::*;
    let files = [
        file("src/a.spec.ts", New),
        file("src/b.test.tsx", Modified),
        file("e2e/login.ts", New),
        file(".foundry/reports/s.ada.json", New),
    ];
    assert_eq!(type_of("Login", "x", &files).0, CommitType::Test);
    assert_eq!(
        type_of(
            "Login",
            "x",
            &[file("src/a.ts", New), file("src/a.spec.ts", New)]
        )
        .0,
        CommitType::Feat
    );
}

#[test]
fn only_docs_or_markdown_is_docs() {
    use FileChange::*;
    let files = [file("docs/guide.txt", New), file("README.md", Modified)];
    assert_eq!(
        type_of("Login", "x", &files),
        (CommitType::Docs, "only docs or markdown changed".into())
    );
}

#[test]
fn only_config_lockfiles_or_tooling_or_the_foundation_slice_is_chore() {
    use FileChange::*;
    let files = [
        file("package.json", Modified),
        file("pnpm-lock.yaml", Modified),
        file("vite.config.ts", New),
        file("tsconfig.app.json", New),
        file(".github/workflows/ci.yml", New),
        file("Cargo.lock", Modified),
    ];
    assert_eq!(
        type_of("Login", "x", &files),
        (
            CommitType::Chore,
            "only config, lockfiles or tooling changed".into()
        )
    );
    assert_eq!(
        type_of("Foundation", "Set up the app", &[file("src/main.ts", New)]),
        (CommitType::Chore, "it's the foundation slice".into())
    );
    assert_eq!(
        type_of("Login", "x", &[file(".foundry/reviews/s.json", New)]).0,
        CommitType::Chore
    );
}

#[test]
fn a_name_or_deliverable_starting_with_a_fix_word_is_fix() {
    use FileChange::*;
    let files = [file("src/a.ts", Modified)];
    assert_eq!(
        type_of("Fix login redirect", "x", &files),
        (CommitType::Fix, "the slice name starts with \"fix\"".into())
    );
    for deliverable in [
        "Bug: crash on save",
        "Repair the cart",
        "resolves #12 sorting",
    ] {
        assert_eq!(
            type_of("Cart", deliverable, &files).0,
            CommitType::Fix,
            "{deliverable}"
        );
    }
    assert_eq!(
        type_of("Fixtures", "x", &files).0,
        CommitType::Feat,
        "not a fix word"
    );
}

#[test]
fn refactor_without_new_feature_files_is_refactor_and_with_them_is_feat() {
    use FileChange::*;
    let moved = [
        file("src/features/cart/a.ts", Modified),
        file("src/lib/b.ts", New),
    ];
    assert_eq!(type_of("Cart cleanup", "x", &moved).0, CommitType::Refactor);
    assert_eq!(
        type_of("Cart", "Restructure the store", &moved).0,
        CommitType::Refactor
    );
    let added = [file("src/features/cart/new.ts", New)];
    let (t, reason) = type_of("Refactor cart", "x", &added);
    assert_eq!(t, CommitType::Feat);
    assert!(
        reason.contains("adds new files in a feature folder"),
        "{reason}"
    );
}

#[test]
fn anything_else_is_feat() {
    use FileChange::*;
    assert_eq!(
        type_of("Login", "Users can log in", &[file("src/login.ts", New)]),
        (CommitType::Feat, "new or changed product code".into())
    );
}
