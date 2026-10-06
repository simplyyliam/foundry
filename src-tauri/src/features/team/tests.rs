use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ::image::{DynamicImage, GenericImageView, ImageFormat, RgbImage};
use base64::Engine as _;
use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::testing::TestRepo;

const P: &str = "project-1";

// ---------- fixtures ----------

/// Replies in order; records prompts. An empty queue is a runner failure.
#[derive(Default)]
struct FakeRunner {
    replies: Mutex<VecDeque<Result<String, AppError>>>,
    prompts: Mutex<Vec<String>>,
}

impl LlmRunner for FakeRunner {
    fn run(&self, prompt: &str, _control: &RunControl) -> Result<String, AppError> {
        self.prompts.lock().unwrap().push(prompt.to_string());
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(AppError::internal("no more replies")))
    }
}

struct Env {
    repo: TestRepo,
    data: TempDir,
    runner: Arc<FakeRunner>,
    store: TeamStore,
}

impl Env {
    fn new() -> Self {
        Self::with(TestRepo::new(), tempfile::tempdir().unwrap())
    }

    fn with(repo: TestRepo, data: TempDir) -> Self {
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let runner = Arc::new(FakeRunner::default());
        let source = Arc::clone(&runner);
        let store = TeamStore::new(
            data.path(),
            repos,
            move || Ok(Arc::clone(&source) as Arc<dyn LlmRunner>),
            || Duration::from_secs(5),
        );
        Self {
            repo,
            data,
            runner,
            store,
        }
    }

    fn with_plan(self) -> Self {
        self.repo.write(
            PLAN_FILE,
            &json!({
                "version": 1, "revision": 3,
                "slices": [
                    { "id": "s1", "name": "Library", "delivers": ["save a reading list"], "dependsOn": [] },
                    { "id": "s2", "name": "Offline", "delivers": ["read offline"], "dependsOn": ["s1"] }
                ],
                "requirements": [], "deferred": []
            })
            .to_string(),
        );
        self
    }

    fn replies(&self, replies: Vec<Result<String, AppError>>) {
        *self.runner.replies.lock().unwrap() = replies.into();
    }

    fn read(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.repo.path().join(rel)).ok()
    }
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(root, &path, out);
            } else {
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn proposal(agents: Value) -> String {
    json!({ "agents": agents }).to_string()
}

fn valid_proposal() -> String {
    proposal(json!([
        { "tempId": "a1", "handbookRoleId": "full-stack-engineer", "customRole": null,
          "displayName": "Ada", "title": "Full-Stack Engineer",
          "rationale": "Builds both slices.", "relatedSliceIds": ["s1", "s2"] },
        { "tempId": "a2", "handbookRoleId": null,
          "customRole": { "title": "Offline Specialist", "description": "Sync and caching",
                          "skillTemplate": "You make the app work without a network." },
          "displayName": "Otto", "title": "Offline Specialist",
          "rationale": "Offline is the hard part.", "relatedSliceIds": ["s2"] }
    ]))
}

fn handbook_agent(name: &str, role: &str, slices: &[&str]) -> RosterAgent {
    RosterAgent {
        handbook_role_id: Some(role.into()),
        display_name: name.into(),
        related_slice_ids: slices.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn custom_agent(name: &str) -> RosterAgent {
    RosterAgent {
        custom_role: Some(CustomRole {
            title: "Offline Specialist".into(),
            description: "Sync and caching".into(),
            skill_template: "You make the app work without a network, and prove it.".into(),
        }),
        display_name: name.into(),
        related_slice_ids: vec!["s2".into()],
        ..Default::default()
    }
}

const HEADINGS: [&str; 5] = [
    "## Reads",
    "## Writes",
    "## Hands off",
    "## Done means",
    "## When blocked",
];

fn assert_five_headings(text: &str) {
    let found: Vec<&str> = text.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(found, HEADINGS, "exactly the five headings, in order");
}

// ---------- handbook ----------

#[test]
fn the_starter_handbook_has_twelve_real_roles_and_is_editable() {
    let env = Env::new();
    let roles = env.store.handbook().unwrap();
    let titles: Vec<&str> = roles.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Product Lead",
            "Full-Stack Engineer",
            "Frontend Engineer",
            "Backend Engineer",
            "Mobile Engineer",
            "DevOps/Infra Engineer",
            "QA/Test Engineer",
            "Security Engineer",
            "Data Engineer",
            "Technical Writer",
            "UI/UX Designer",
            "Manager"
        ]
    );
    for role in &roles {
        let words = role.skill_template.split_whitespace().count();
        assert!(
            words >= 70,
            "{}: skillTemplate is thin ({words} words)",
            role.title
        );
        for floor in PROTECTED_FLOOR {
            assert!(
                role.typical_protected_globs.iter().any(|g| g == floor),
                "{} {floor}",
                role.id
            );
        }
    }

    let added = env
        .store
        .add_role(NewHandbookRole {
            title: "Product Lead".into(),
            skill_template: "Another one.".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(added.last().unwrap().id, "product-lead-2");
    env.store
        .update_role(
            "manager",
            HandbookRolePatch {
                title: Some("Coordinator".into()),
                ..Default::default()
            },
        )
        .unwrap();
    env.store.remove_role("mobile-engineer").unwrap();

    // A deleted starter role stays deleted (no re-seeding).
    let reopened = Env::with(TestRepo::new(), env.data);
    let roles = reopened.store.handbook().unwrap();
    assert!(!roles.iter().any(|r| r.id == "mobile-engineer"));
    assert!(roles.iter().any(|r| r.title == "Coordinator"));
}

// ---------- R-4-10 ----------

/// Property: whatever the plan and whatever the model replies, compose_team leaves the repo
/// and the app-data folder exactly as they were (it doesn't even seed the handbook).
#[test]
fn compose_team_writes_nothing_under_any_outcome() {
    let plans: Vec<Option<&str>> = vec![
        None,
        Some("{ not json"),
        Some(r#"{ "slices": "nope" }"#),
        Some(r#"{ "slices": [{ "name": "no id" }] }"#),
        Some(
            r#"{ "slices": [{ "id": "s1", "name": "Library", "delivers": [] },
                            { "id": "s2", "name": "Offline", "delivers": [] }] }"#,
        ),
    ];
    let replies: Vec<Vec<Result<String, AppError>>> = vec![
        vec![],
        vec![Ok("garbage".into()), Ok("```json\n{]\n```".into())],
        vec![
            Ok(proposal(
                json!([{ "tempId": "x", "handbookRoleId": "wizard", "customRole": null,
            "displayName": "W", "title": "Wizard", "rationale": "", "relatedSliceIds": [] }])
            ));
            2
        ],
        vec![Ok(proposal(json!([]))), Ok(valid_proposal())],
        vec![Ok(valid_proposal())],
        vec![Err(AppError::new(ErrorCode::TimedOut, "slow"))],
    ];
    let mut outcomes = (0, 0);
    for plan in &plans {
        for reply in &replies {
            let env = Env::new();
            if let Some(plan) = plan {
                env.repo.write(PLAN_FILE, plan);
            }
            env.replies(reply.clone());
            let (repo_before, data_before) = (snapshot(env.repo.path()), snapshot(env.data.path()));

            let result = env.store.compose(P);

            assert_eq!(
                snapshot(env.repo.path()),
                repo_before,
                "repo changed: {plan:?} {reply:?}"
            );
            assert_eq!(
                snapshot(env.data.path()),
                data_before,
                "app data changed: {plan:?}"
            );
            if result.is_ok() {
                outcomes.0 += 1;
            } else {
                outcomes.1 += 1;
            }
        }
    }
    assert!(
        outcomes.0 > 0 && outcomes.1 > 0,
        "both successes and failures covered: {outcomes:?}"
    );
}

// ---------- R-4-11 ----------

#[test]
fn a_proposal_with_an_unknown_handbook_role_never_reaches_the_human() {
    let env = Env::new().with_plan();
    let unknown = proposal(
        json!([{ "tempId": "x", "handbookRoleId": "wizard", "customRole": null,
        "displayName": "Merlin", "title": "Wizard", "rationale": "magic", "relatedSliceIds": ["s1"] }]),
    );

    env.replies(vec![Ok(unknown.clone()), Ok(unknown.clone())]);
    let err = env.store.compose(P).unwrap_err();
    assert_eq!(err.code, ErrorCode::GenerationFailed);
    assert!(
        err.message
            .contains("\"wizard\" is not in the Role Handbook"),
        "{}",
        err.message
    );

    // One retry, with the error in the prompt; a fixed reply then gets through.
    env.replies(vec![Ok(unknown), Ok(valid_proposal())]);
    let proposal = env.store.compose(P).unwrap();
    assert_eq!(proposal.agents.len(), 2);
    let prompts = env.runner.prompts.lock().unwrap().clone();
    assert!(prompts
        .last()
        .unwrap()
        .contains("\"wizard\" is not in the Role Handbook"));
    assert!(
        prompts[0].contains("\"full-stack-engineer\""),
        "the handbook is in the prompt"
    );
    assert!(prompts[0].contains("\"s2\""), "the plan is in the prompt");
}

#[test]
fn proposals_are_checked_for_slices_and_role_shape() {
    let env = Env::new().with_plan();
    let bad = proposal(json!([
        { "tempId": "a", "handbookRoleId": "qa-test-engineer", "customRole": { "title": "x", "description": "", "skillTemplate": "y" },
          "displayName": "Both", "title": "T", "rationale": "", "relatedSliceIds": ["s9"] },
        { "tempId": "a", "handbookRoleId": null, "customRole": null,
          "displayName": "", "title": "", "rationale": "", "relatedSliceIds": [] }
    ]));
    env.replies(vec![Ok(bad.clone()), Ok(bad)]);
    let message = env.store.compose(P).unwrap_err().message;
    for expected in [
        "agents[0] must have exactly one of handbookRoleId and customRole",
        "agents[0].relatedSliceIds: \"s9\" is not a plan slice",
        "agents[1].tempId must be present and unique",
        "agents[1].displayName must not be empty",
    ] {
        assert!(
            message.contains(expected),
            "missing {expected:?} in {message}"
        );
    }
}

// ---------- R-4-12 ----------

#[test]
fn confirm_team_writes_an_entry_and_a_five_heading_file_for_every_agent() {
    let env = Env::new().with_plan();

    let written = env
        .store
        .confirm(
            P,
            vec![
                handbook_agent("Ada", "full-stack-engineer", &["s1", "s2"]),
                custom_agent("Otto"),
            ],
        )
        .unwrap();

    assert_eq!(written.created, ["ada", "otto"]);
    let roles: Value = serde_json::from_str(&env.read(ROLES_FILE).unwrap()).unwrap();
    let agents = roles["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 2);
    assert_eq!(agents[0]["id"], "ada");
    assert_eq!(agents[0]["sourceHandbookRoleId"], "full-stack-engineer");
    assert_eq!(agents[0]["title"], "Full-Stack Engineer");
    assert_eq!(agents[0]["writeGlobs"][0], "packages/frontend/**");
    assert_eq!(agents[1]["id"], "otto");
    assert!(
        agents[1]["sourceHandbookRoleId"].is_null(),
        "a purely custom agent"
    );
    assert_eq!(agents[1]["writeGlobs"], json!([]));
    for key in [
        "id",
        "displayName",
        "title",
        "avatarPath",
        "reads",
        "writeGlobs",
        "protectedGlobs",
        "sourceHandbookRoleId",
    ] {
        assert!(agents[0].get(key).is_some(), "{key}");
    }

    let ada = env.read("agents/roles/ada.md").unwrap();
    assert_five_headings(&ada);
    assert!(ada.starts_with("# Ada: Full-Stack Engineer\n"));
    assert!(
        ada.contains("You build complete vertical slices"),
        "handbook skill merged"
    );
    assert!(
        ada.contains("- Library (`s1`): delivers save a reading list"),
        "slices merged"
    );
    assert!(ada.contains("- Offline (`s2`)"));
    let otto = env.read("agents/roles/otto.md").unwrap();
    assert_five_headings(&otto);
    assert!(otto.contains("You make the app work without a network, and prove it."));
    assert!(
        otto.contains("Nothing: this role advises and coordinates"),
        "no write globs"
    );

    // A confirmed team can't be re-proposed or re-confirmed (AGENTS.md, Law 6).
    assert_eq!(
        env.store.compose(P).unwrap_err().code,
        ErrorCode::TeamConfirmed
    );
    let again = env.store.confirm(P, vec![custom_agent("Zed")]);
    assert_eq!(again.unwrap_err().code, ErrorCode::TeamConfirmed);
    assert_eq!(env.store.team(P).unwrap().unwrap().agents.len(), 2);
}

#[test]
fn confirm_team_validates_the_roster() {
    let env = Env::new().with_plan();
    let both = RosterAgent {
        handbook_role_id: Some("manager".into()),
        ..custom_agent("Both")
    };
    for (roster, code) in [
        (vec![], ErrorCode::InvalidInput),
        (
            vec![handbook_agent("X", "wizard", &[])],
            ErrorCode::NotFound,
        ),
        (
            vec![handbook_agent("X", "manager", &["s9"])],
            ErrorCode::NotFound,
        ),
        (
            vec![handbook_agent("  ", "manager", &[])],
            ErrorCode::InvalidInput,
        ),
        (vec![both], ErrorCode::InvalidInput),
    ] {
        assert_eq!(env.store.confirm(P, roster).unwrap_err().code, code);
        assert!(
            env.read(ROLES_FILE).is_none(),
            "nothing written on a refusal"
        );
    }
    // Same display names get distinct ids.
    let written = env
        .store
        .confirm(
            P,
            vec![
                handbook_agent("Ada", "manager", &[]),
                handbook_agent("Ada", "manager", &[]),
            ],
        )
        .unwrap();
    assert_eq!(written.created, ["ada", "ada-2"]);
}

// ---------- R-4-13 ----------

#[test]
fn the_protected_floor_is_always_there_and_restoring_it_is_reported() {
    let env = Env::new().with_plan();
    // A roster edit that drops most of the floor, and one that keeps the defaults.
    let shrunk = RosterAgent {
        protected_globs: Some(vec!["AGENTS.md".into(), "secrets/**".into()]),
        ..handbook_agent("Shrink", "backend-engineer", &[])
    };
    // The handbook itself edited to drop the floor.
    env.store
        .update_role(
            "qa-test-engineer",
            HandbookRolePatch {
                typical_protected_globs: Some(vec![]),
                ..Default::default()
            },
        )
        .unwrap();

    let written = env
        .store
        .confirm(
            P,
            vec![
                shrunk,
                handbook_agent("Keep", "manager", &[]),
                handbook_agent("Qa", "qa-test-engineer", &[]),
            ],
        )
        .unwrap();

    for agent in &written.team.agents {
        for floor in PROTECTED_FLOOR {
            assert!(
                agent.protected_globs.iter().any(|g| g == floor),
                "{} lacks {floor}",
                agent.id
            );
        }
    }
    let shrink = written
        .team
        .agents
        .iter()
        .find(|a| a.id == "shrink")
        .unwrap();
    assert!(
        shrink.protected_globs.contains(&"secrets/**".to_string()),
        "extras kept"
    );
    // Reported in roster order; "Keep" (handbook defaults, which include the floor) needed
    // nothing restored.
    let all_floor: Vec<String> = PROTECTED_FLOOR.iter().map(|g| g.to_string()).collect();
    assert_eq!(
        written.floor_restored,
        [
            FloorRestored {
                agent_id: "shrink".into(),
                globs: all_floor[1..].to_vec(),
            },
            FloorRestored {
                agent_id: "qa".into(),
                globs: all_floor,
            },
        ]
    );
    // The role file lists the floor as protected too.
    let file = env.read("agents/roles/shrink.md").unwrap();
    assert!(file.contains("- `.foundry/plan.json`") && file.contains("- `.claude/**`"));

    // The same floor applies to add_agent_manually.
    let added = env
        .store
        .add_manually(
            P,
            RosterAgent {
                protected_globs: Some(vec![]),
                ..custom_agent("Late")
            },
        )
        .unwrap();
    assert_eq!(added.floor_restored.len(), 1);
    assert_eq!(added.floor_restored[0].agent_id, "late");
}

// ---------- R-4-14 ----------

#[test]
fn identical_rosters_give_byte_identical_files() {
    let roster = || {
        vec![
            handbook_agent("Ada", "full-stack-engineer", &["s1"]),
            custom_agent("Otto"),
        ]
    };
    let (one, two) = (Env::new().with_plan(), Env::new().with_plan());
    one.store.confirm(P, roster()).unwrap();
    two.store.confirm(P, roster()).unwrap();
    for file in [ROLES_FILE, "agents/roles/ada.md", "agents/roles/otto.md"] {
        assert_eq!(one.read(file), two.read(file), "{file}");
    }
}

// ---------- R-4-15 ----------

#[test]
fn add_agent_manually_leaves_every_existing_agent_untouched() {
    let env = Env::new().with_plan();
    assert_eq!(
        env.store
            .add_manually(P, custom_agent("Early"))
            .unwrap_err()
            .code,
        ErrorCode::TeamNotConfirmed
    );
    env.store
        .confirm(
            P,
            vec![
                handbook_agent("Ada", "full-stack-engineer", &["s1"]),
                custom_agent("Otto"),
            ],
        )
        .unwrap();
    let files_before = (
        env.read("agents/roles/ada.md"),
        env.read("agents/roles/otto.md"),
    );
    let entries_before = env.store.team(P).unwrap().unwrap().agents;

    let added = env
        .store
        .add_manually(P, handbook_agent("Ada", "technical-writer", &[]))
        .unwrap();

    assert_eq!(
        added.created,
        ["ada-2"],
        "a free id beside the existing Ada"
    );
    assert_eq!(
        (
            env.read("agents/roles/ada.md"),
            env.read("agents/roles/otto.md")
        ),
        files_before
    );
    let entries_after = env.store.team(P).unwrap().unwrap().agents;
    for entry in &entries_before {
        assert_eq!(
            entries_after.iter().find(|e| e.id == entry.id),
            Some(entry),
            "{}",
            entry.id
        );
    }
    assert_eq!(entries_after.len(), 3);
    assert_five_headings(&env.read("agents/roles/ada-2.md").unwrap());
}

// ---------- R-4-16 ----------

fn photo(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, ::image::Rgb([200, 30, 30])))
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn an_uploaded_avatar_gets_bakery_processing_and_a_builtin_needs_none() {
    let env = Env::new().with_plan();
    env.store.confirm(P, vec![custom_agent("Otto")]).unwrap();
    let upload = photo(2000, 1000);

    let agent = env
        .store
        .set_avatar(
            P,
            "otto",
            AvatarChoice {
                builtin_id: None,
                bytes: Some(upload.clone()),
            },
        )
        .unwrap();

    assert_eq!(
        agent.avatar_path.as_deref(),
        Some(".foundry/agents/otto-avatar.png")
    );
    let stored = fs::read(env.repo.path().join(".foundry/agents/otto-avatar.png")).unwrap();
    // Exactly what the Bakery's processing produces from the same input.
    assert_eq!(stored, image::process(&upload).unwrap().bytes);
    assert_eq!(
        ::image::load_from_memory(&stored).unwrap().dimensions(),
        (1600, 800)
    );
    assert_eq!(
        env.store.team(P).unwrap().unwrap().agents[0]
            .avatar_path
            .as_deref(),
        Some(".foundry/agents/otto-avatar.png")
    );

    // A built-in choice: its (undecodable) bytes are never processed or copied.
    let builtin = env
        .store
        .set_avatar(
            P,
            "otto",
            AvatarChoice {
                builtin_id: Some("test-fixture".into()),
                bytes: None,
            },
        )
        .unwrap();
    assert_eq!(builtin.avatar_path.as_deref(), Some("builtin:test-fixture"));
    assert_eq!(TeamStore::builtin_avatars(), ["test-fixture"]);

    for choice in [
        AvatarChoice {
            builtin_id: None,
            bytes: None,
        },
        AvatarChoice {
            builtin_id: Some("x".into()),
            bytes: Some(upload.clone()),
        },
    ] {
        assert_eq!(
            env.store.set_avatar(P, "otto", choice).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
    let unknown = AvatarChoice {
        builtin_id: Some("nope".into()),
        bytes: None,
    };
    assert_eq!(
        env.store.set_avatar(P, "otto", unknown).unwrap_err().code,
        ErrorCode::NotFound
    );
    let not_image = AvatarChoice {
        builtin_id: None,
        bytes: Some(b"text".to_vec()),
    };
    assert_eq!(
        env.store.set_avatar(P, "otto", not_image).unwrap_err().code,
        ErrorCode::UnsupportedImage
    );
}

// ---------- agent_avatar ----------

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, ::image::Rgb([30, 30, 200])))
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)
        .unwrap();
    out
}

fn upload(env: &Env, agent_id: &str, bytes: Vec<u8>) -> String {
    env.store
        .set_avatar(
            P,
            agent_id,
            AvatarChoice {
                builtin_id: None,
                bytes: Some(bytes),
            },
        )
        .unwrap()
        .avatar_path
        .unwrap()
}

/// Splits a data URL into its media type and decoded bytes.
fn decode_data_url(url: &str) -> (String, Vec<u8>) {
    let rest = url.strip_prefix("data:").expect("a data URL");
    let (mime, b64) = rest.split_once(";base64,").expect("base64 data URL");
    (
        mime.to_string(),
        STANDARD.decode(b64).expect("valid base64"),
    )
}

/// Hand-edits `roles.json`, as a user could, to point an agent's avatar anywhere.
fn hand_edit_avatar(env: &Env, agent_id: &str, avatar_path: &str) {
    let mut roles: Value = serde_json::from_str(&env.read(ROLES_FILE).unwrap()).unwrap();
    for agent in roles["agents"].as_array_mut().unwrap() {
        if agent["id"] == agent_id {
            agent["avatarPath"] = json!(avatar_path);
        }
    }
    env.repo.write(ROLES_FILE, &roles.to_string());
}

#[test]
fn an_uploaded_avatar_comes_back_as_a_data_url_at_most_256px_in_its_stored_format() {
    let env = Env::new().with_plan();
    env.store
        .confirm(P, vec![custom_agent("Otto"), custom_agent("Ada")])
        .unwrap();

    upload(&env, "otto", photo(2000, 1000));
    let (mime, bytes) = decode_data_url(&env.store.avatar(P, "otto").unwrap().unwrap());
    assert_eq!(mime, "image/png");
    assert_eq!(::image::guess_format(&bytes).unwrap(), ImageFormat::Png);
    assert_eq!(
        ::image::load_from_memory(&bytes).unwrap().dimensions(),
        (256, 128)
    );

    upload(&env, "ada", jpeg(300, 900));
    let (mime, bytes) = decode_data_url(&env.store.avatar(P, "ada").unwrap().unwrap());
    assert_eq!(mime, "image/jpeg");
    assert_eq!(::image::guess_format(&bytes).unwrap(), ImageFormat::Jpeg);
    let (w, h) = ::image::load_from_memory(&bytes).unwrap().dimensions();
    assert_eq!(h, 256);
    assert!(w < 256, "{w}x{h} keeps the aspect ratio");

    // Already small enough: served byte-identical to what's stored, never upscaled.
    let rel = upload(&env, "ada", photo(64, 32));
    let (_, bytes) = decode_data_url(&env.store.avatar(P, "ada").unwrap().unwrap());
    assert_eq!(bytes, fs::read(env.repo.path().join(rel)).unwrap());
}

#[test]
fn no_avatar_or_a_missing_file_returns_null() {
    let env = Env::new().with_plan();
    env.store.confirm(P, vec![custom_agent("Otto")]).unwrap();
    assert_eq!(env.store.avatar(P, "otto").unwrap(), None);

    let rel = upload(&env, "otto", photo(64, 64));
    fs::remove_file(env.repo.path().join(&rel)).unwrap();
    assert_eq!(env.store.avatar(P, "otto").unwrap(), None);

    // The whole folder gone is missing too.
    fs::remove_dir_all(env.repo.path().join(AVATAR_DIR)).unwrap();
    assert_eq!(env.store.avatar(P, "otto").unwrap(), None);

    assert_eq!(
        env.store.avatar(P, "nobody").unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn an_avatar_path_escaping_foundry_agents_is_refused() {
    let env = Env::new().with_plan();
    env.store.confirm(P, vec![custom_agent("Otto")]).unwrap();
    upload(&env, "otto", photo(64, 64));
    // Real images outside the folder, so only the path check can stop them.
    fs::write(env.repo.path().join("elsewhere.png"), photo(8, 8)).unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.png"), photo(8, 8)).unwrap();
    crate::testing::link_dir(
        outside.path(),
        // Joined per component: mklink /J rejects forward slashes.
        &env.repo
            .path()
            .join(".foundry")
            .join("agents")
            .join("escape"),
    );

    let absolute = outside.path().join("secret.png");
    for path in [
        "elsewhere.png",
        "agents/roles.json",
        ".foundry/agents",
        ".foundry/agents/../../elsewhere.png",
        "../elsewhere.png",
        ".foundry/agents/escape/secret.png",
        absolute.to_str().unwrap(),
    ] {
        hand_edit_avatar(&env, "otto", path);
        assert_eq!(
            env.store.avatar(P, "otto").unwrap_err().code,
            ErrorCode::PathEscape,
            "{path}"
        );
    }
}

#[test]
fn a_builtin_avatar_resolves_from_the_embedded_set_and_an_unknown_id_is_null() {
    let env = Env::new().with_plan();
    env.store.confirm(P, vec![custom_agent("Otto")]).unwrap();
    env.store
        .set_avatar(
            P,
            "otto",
            AvatarChoice {
                builtin_id: Some("test-fixture".into()),
                bytes: None,
            },
        )
        .unwrap();
    // The embedded bytes as they are: the fixture isn't decodable, so it can't have been
    // run through image processing.
    let (mime, bytes) = decode_data_url(&env.store.avatar(P, "otto").unwrap().unwrap());
    assert_eq!(mime, "image/png");
    assert_eq!(bytes, b"not an image");

    hand_edit_avatar(&env, "otto", "builtin:nope");
    assert_eq!(env.store.avatar(P, "otto").unwrap(), None);
}

#[test]
fn agent_avatar_writes_nothing() {
    let env = Env::new().with_plan();
    env.store
        .confirm(P, vec![custom_agent("Otto"), custom_agent("Ada")])
        .unwrap();
    upload(&env, "otto", photo(2000, 1000));
    upload(&env, "ada", jpeg(64, 64));
    let before = (snapshot(env.repo.path()), snapshot(env.data.path()));

    for agent in ["otto", "ada"] {
        assert!(env.store.avatar(P, agent).unwrap().is_some());
    }
    assert_eq!(
        (snapshot(env.repo.path()), snapshot(env.data.path())),
        before
    );

    // A refused path writes nothing either.
    hand_edit_avatar(&env, "ada", "../elsewhere.png");
    let before = (snapshot(env.repo.path()), snapshot(env.data.path()));
    env.store.avatar(P, "ada").unwrap_err();
    assert_eq!(
        (snapshot(env.repo.path()), snapshot(env.data.path())),
        before
    );
}

// ---------- R-4-17 (team part) ----------

#[test]
fn team_writes_stay_inside_the_allowlist() {
    let env = Env::new().with_plan();
    env.repo.write("src/main.rs", "fn main() {}");
    let before = snapshot(env.repo.path());

    env.store
        .confirm(
            P,
            vec![handbook_agent("Ada", "full-stack-engineer", &["s1"])],
        )
        .unwrap();
    env.store.add_manually(P, custom_agent("Otto")).unwrap();
    env.store
        .set_avatar(
            P,
            "otto",
            AvatarChoice {
                builtin_id: None,
                bytes: Some(photo(64, 64)),
            },
        )
        .unwrap();

    for (path, bytes) in snapshot(env.repo.path()) {
        if path.ends_with('/') || before.get(&path) == Some(&bytes) {
            continue;
        }
        assert!(
            repo_write::is_allowed(Path::new(&path)),
            "{path} is outside the allowlist"
        );
    }
}

// ---------- role_handbook_add_from_paths ----------

/// A folder of dropped files, outside the repo and the app data.
struct Drops(TempDir);

impl Drops {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }

    fn file(&self, name: &str, text: impl AsRef<[u8]>) -> String {
        let path = self.0.path().join(name);
        fs::write(&path, text).unwrap();
        path.to_string_lossy().into_owned()
    }
}

fn role<'a>(roles: &'a [HandbookRole], id: &str) -> &'a HandbookRole {
    roles
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no role {id}"))
}

#[test]
fn frontmatter_is_parsed_with_inline_and_block_lists() {
    let env = Env::new();
    let drops = Drops::new();
    let inline = drops.file(
        "inline.md",
        "\u{feff}---\r\ntitle: Release Manager\r\ndescription: \"Cuts releases: tags, notes\"\r\n\
         writes: [CHANGELOG.md, \"release/**\"]\r\nprotected: ['src/**']\r\n---\r\n\
         You ship releases.\r\n\r\nYou never skip the changelog.\r\n",
    );
    let block = drops.file(
        "block.md",
        "---\n# a comment\ntitle: Data Steward\nwrites:\n  - data/**\n  - \"docs/data/**\"\n\
         protected:\n- migrations/**\n---\nYou look after the data.\n",
    );

    let imported = env
        .store
        .add_roles_from_paths(&[inline, block], false)
        .unwrap();
    assert_eq!(imported.added, ["release-manager", "data-steward"]);
    assert!(imported.replaced.is_empty());

    let roles = env.store.handbook().unwrap();
    let release = role(&roles, "release-manager");
    assert_eq!(release.title, "Release Manager");
    assert_eq!(release.description, "Cuts releases: tags, notes");
    assert_eq!(release.typical_write_globs, ["CHANGELOG.md", "release/**"]);
    assert_eq!(release.typical_protected_globs, ["src/**"]);
    assert_eq!(
        release.skill_template, "You ship releases.\n\nYou never skip the changelog.",
        "LF only, no BOM, no frontmatter"
    );
    let data = role(&roles, "data-steward");
    assert_eq!(data.typical_write_globs, ["data/**", "docs/data/**"]);
    assert_eq!(data.typical_protected_globs, ["migrations/**"]);
    assert_eq!(data.description, "You look after the data.");
}

#[test]
fn without_frontmatter_or_a_title_the_heading_then_the_file_name_is_the_title() {
    let env = Env::new();
    let drops = Drops::new();
    let heading = drops.file(
        "whatever.md",
        "Some intro.\n\n# Accessibility Lead #\n\nYou own a11y.\n",
    );
    let untitled = drops.file(
        "growth-hacker.md",
        "---\ndescription: Grows things\n---\n## Not a title\n\nYou run experiments.\n",
    );

    let imported = env
        .store
        .add_roles_from_paths(&[heading, untitled], false)
        .unwrap();
    assert_eq!(imported.added, ["accessibility-lead", "growth-hacker"]);
    let roles = env.store.handbook().unwrap();
    assert_eq!(
        role(&roles, "accessibility-lead").title,
        "Accessibility Lead"
    );
    assert_eq!(role(&roles, "growth-hacker").title, "growth-hacker");
    assert_eq!(role(&roles, "growth-hacker").description, "Grows things");
}

#[test]
fn a_missing_description_is_the_first_non_heading_paragraph_cut_at_200_characters() {
    let env = Env::new();
    let drops = Drops::new();
    let long = "word ".repeat(100);
    let file = drops.file(
        "writer.md",
        format!("# Copywriter\n\n## Voice\n\n```\ncode is skipped\n```\n\n{long}\nsecond line\n\nLater paragraph.\n"),
    );
    env.store.add_roles_from_paths(&[file], false).unwrap();
    let roles = env.store.handbook().unwrap();
    let description = &role(&roles, "copywriter").description;
    assert_eq!(
        description.chars().count(),
        199,
        "200 characters, trailing space trimmed"
    );
    assert!(long.starts_with(description.as_str()), "{description}");

    let short = drops.file("short.md", "# Short\nOne line.\nTwo lines.\n\nNext.\n");
    env.store.add_roles_from_paths(&[short], false).unwrap();
    let roles = env.store.handbook().unwrap();
    assert_eq!(role(&roles, "short").description, "One line. Two lines.");
}

#[test]
fn any_refused_file_refuses_the_whole_call_and_names_it() {
    let env = Env::new();
    let drops = Drops::new();
    let good = drops.file("good.md", "# Good Role\n\nFine.\n");
    let before = env.store.handbook().unwrap();
    let handbook_bytes =
        || fs::read(env.data.path().join(HANDBOOK_DIR).join(HANDBOOK_FILE)).unwrap();
    let on_disk = handbook_bytes();

    let cases: Vec<(String, ErrorCode, &str)> = vec![
        (
            drops.file("notes.txt", "# Not markdown\n"),
            ErrorCode::UnsupportedFile,
            "notes.txt",
        ),
        (
            drops.file("huge.md", "x".repeat(100 * 1024 + 1)),
            ErrorCode::FileTooLarge,
            "huge.md",
        ),
        (
            drops.file(
                "bad.md",
                "---\ntitle: Bad\nthis line is wrong\n---\nBody.\n",
            ),
            ErrorCode::InvalidInput,
            "bad.md, line 3",
        ),
        (
            drops.file("unknown.md", "---\ntitle: X\nowner: me\n---\n"),
            ErrorCode::InvalidInput,
            "unknown.md, line 3",
        ),
        (
            drops.file("open.md", "---\ntitle: X\n"),
            ErrorCode::InvalidInput,
            "open.md, line 1",
        ),
        (
            drops.file("stray.md", "---\ntitle: X\n- item\n---\n"),
            ErrorCode::InvalidInput,
            "stray.md, line 3",
        ),
    ];
    for (bad, code, names) in cases {
        let err = env
            .store
            .add_roles_from_paths(&[good.clone(), bad], false)
            .unwrap_err();
        assert_eq!(err.code, code, "{}", err.message);
        assert!(err.message.contains(names), "{names}: {}", err.message);
        assert_eq!(env.store.handbook().unwrap(), before, "nothing added");
        assert_eq!(handbook_bytes(), on_disk);
    }

    let too_many: Vec<String> = (0..51)
        .map(|i| drops.file(&format!("r{i}.md"), format!("# Role {i}\n")))
        .collect();
    let err = env
        .store
        .add_roles_from_paths(&too_many, false)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(env.store.handbook().unwrap(), before);
    let fifty = env.store.add_roles_from_paths(&too_many[..50], false);
    assert_eq!(fifty.unwrap().added.len(), 50);
}

#[test]
fn a_duplicate_without_replace_is_role_exists_and_changes_nothing() {
    let env = Env::new();
    let drops = Drops::new();
    let new = drops.file("new.md", "# Brand New\n");
    let dup = drops.file("lead.md", "---\ntitle: Product Lead\n---\nMine.\n");
    let before = env.store.handbook().unwrap();

    let err = env
        .store
        .add_roles_from_paths(&[new.clone(), dup], false)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::RoleExists);
    assert_eq!(serde_json::to_value(&err).unwrap()["code"], "role_exists");
    assert!(err.message.contains("Product Lead"), "{}", err.message);
    assert_eq!(env.store.handbook().unwrap(), before);

    // Two files in one call that would be the same role are refused too.
    let twin = drops.file("twin.md", "# Brand New\n");
    let err = env
        .store
        .add_roles_from_paths(&[new, twin], false)
        .unwrap_err();
    assert!(err.message.contains("new.md") && err.message.contains("twin.md"));
    assert_eq!(env.store.handbook().unwrap(), before);
}

#[test]
fn replace_keeps_the_id_and_position_and_takes_the_new_fields() {
    let env = Env::new();
    let drops = Drops::new();
    let before = env.store.handbook().unwrap();
    let at = before
        .iter()
        .position(|r| r.id == "backend-engineer")
        .unwrap();
    let file = drops.file(
        "backend.md",
        "---\ntitle: Backend Engineer\ndescription: APIs only\nwrites: [api/**]\nprotected: []\n---\nYou build APIs.\n",
    );
    let fresh = drops.file("fresh.md", "# Fresh Role\n");

    let imported = env
        .store
        .add_roles_from_paths(&[file, fresh], true)
        .unwrap();
    assert_eq!(imported.replaced, ["backend-engineer"]);
    assert_eq!(imported.added, ["fresh-role"]);

    let roles = env.store.handbook().unwrap();
    assert_eq!(roles.len(), before.len() + 1);
    let backend = &roles[at];
    assert_eq!(backend.id, "backend-engineer");
    assert_eq!(backend.description, "APIs only");
    assert_eq!(backend.typical_write_globs, ["api/**"]);
    assert!(backend.typical_protected_globs.is_empty());
    assert_eq!(backend.skill_template, "You build APIs.");
    assert_eq!(
        roles
            .iter()
            .filter(|r| r.title == "Backend Engineer")
            .count(),
        1
    );
}

#[test]
fn imported_skill_headings_sit_below_the_five_required_headings() {
    let env = Env::new().with_plan();
    let drops = Drops::new();
    let file = drops.file(
        "reviewer.md",
        "---\ntitle: Code Reviewer\n---\n# Code Reviewer\n\nYou review.\n\n## Reads\n\nDiffs.\n\n\
         Setext Two\n----------\n\n#### Deep\n\n###### Deepest\n\n```md\n## Writes in code\n```\n\n- item\n---\n",
    );
    env.store.add_roles_from_paths(&[file], false).unwrap();
    env.store
        .confirm(P, vec![handbook_agent("Rex", "code-reviewer", &["s1"])])
        .unwrap();

    let text = env.read("agents/roles/rex.md").unwrap();
    // Exactly the five at level 2 (the fenced line isn't a heading), and only the file's
    // own title at level 1.
    let outside_code: Vec<&str> = text.lines().filter(|l| *l != "## Writes in code").collect();
    assert_five_headings(&outside_code.join("\n"));
    let level_one: Vec<&str> = text.lines().filter(|l| l.starts_with("# ")).collect();
    assert_eq!(level_one, ["# Rex: Code Reviewer"]);
    for kept in [
        "### Code Reviewer\n",
        "#### Reads\n",
        "#### Setext Two\n",
        "###### Deep\n",
        "###### Deepest\n",
        "```md\n## Writes in code\n```",
        "- item\n---",
    ] {
        assert!(text.contains(kept), "{kept:?} missing in\n{text}");
    }
    assert!(!text.contains("----------"), "setext underline converted");
}

#[test]
fn compose_team_sends_role_summaries_not_skill_bodies() {
    let env = Env::new().with_plan();
    let drops = Drops::new();
    // The first paragraph becomes the description (which is sent); the rest must not be.
    let body = format!(
        "# Long Role\n\nShort intro.\n\n{}",
        "UNIQUE-SKILL-MARKER ".repeat(4000)
    );
    let file = drops.file("long.md", body);
    env.store.add_roles_from_paths(&[file], false).unwrap();

    env.replies(vec![Ok(valid_proposal())]);
    env.store.compose(P).unwrap();
    let prompt = env.runner.prompts.lock().unwrap()[0].clone();
    assert!(!prompt.contains("UNIQUE-SKILL-MARKER"), "no skill body");
    assert!(
        !prompt.contains("You build complete vertical slices"),
        "no starter skill body either"
    );
    assert!(!prompt.contains("typicalProtectedGlobs"));
    let handbook = prompt
        .split("<role_handbook>")
        .nth(1)
        .and_then(|s| s.split("</role_handbook>").next())
        .unwrap();
    let sent: Value = serde_json::from_str(handbook.trim()).unwrap();
    let full_stack = sent
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "full-stack-engineer")
        .unwrap();
    let mut keys: Vec<&String> = full_stack.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["description", "id", "title", "typicalWriteGlobs"]);
    assert_eq!(full_stack["typicalWriteGlobs"][0], "packages/frontend/**");
    assert!(sent
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == "long-role"));
    assert!(prompt.len() < 20_000, "prompt is {} bytes", prompt.len());
}

#[test]
fn dropped_files_are_only_read() {
    let env = Env::new();
    let drops = Drops::new();
    drops.file("a.md", "---\ntitle: A Role\n---\r\nBody.\r\n");
    drops.file("b.md", "\u{feff}# B Role\n");
    drops.file("c.txt", "nope");
    let path = |n: &str| drops.0.path().join(n).to_string_lossy().into_owned();
    let before = snapshot(drops.0.path());

    env.store
        .add_roles_from_paths(&[path("a.md"), path("b.md")], false)
        .unwrap();
    env.store
        .add_roles_from_paths(&[path("a.md")], false)
        .unwrap_err();
    env.store
        .add_roles_from_paths(&[path("a.md"), path("c.txt")], true)
        .unwrap_err();
    env.store
        .add_roles_from_paths(&[path("a.md"), path("b.md")], true)
        .unwrap();

    assert_eq!(
        snapshot(drops.0.path()),
        before,
        "nothing modified or added"
    );
}
