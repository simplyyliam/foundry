use std::fs;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::*;
use crate::testing::TestRepo;

const P: &str = "project-1";

// ---------- fixtures ----------

#[derive(Default)]
struct Revisions(Mutex<Vec<u32>>);

impl PlanEvents for Revisions {
    fn changed(&self, event: PlanChanged) {
        assert_eq!(event.project_id, P);
        self.0.lock().unwrap().push(event.revision);
    }
}

struct Env {
    repo: TestRepo,
    store: Arc<PlanStore>,
    events: Arc<Revisions>,
}

impl Env {
    fn new() -> Self {
        Self::in_repo(TestRepo::new())
    }

    fn in_repo(repo: TestRepo) -> Self {
        repo.write(
            ".foundry/project.json",
            r#"{ "id": "project-1", "name": "Orbit", "tagline": "Read anywhere" }"#,
        );
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let events = Arc::new(Revisions::default());
        let store = PlanStore::new(
            repos,
            Git::system(),
            Arc::clone(&events) as Arc<dyn PlanEvents>,
        );
        Self {
            repo,
            store: Arc::new(store),
            events,
        }
    }

    fn slice(&self, name: &str, deps: &[&str]) -> String {
        self.store
            .add_slice(
                P,
                NewSlice {
                    name: name.into(),
                    delivers: vec![format!("{name} works")],
                    depends_on: deps.iter().map(|d| d.to_string()).collect(),
                },
                None,
            )
            .unwrap()
            .id
    }

    fn requirement(&self, slice: Option<&str>) -> String {
        self.store
            .add_requirement(
                P,
                NewRequirement {
                    text: "It works".into(),
                    kind: RequirementKind::Behavior,
                    slice_id: slice.map(str::to_string),
                    source_excerpt: "it works".into(),
                    verified_by: "a test".into(),
                },
                None,
            )
            .unwrap()
            .id
    }

    fn plan(&self) -> Plan {
        self.store.get(P).unwrap()
    }

    fn slice_view(&self, id: &str) -> SliceView {
        self.plan()
            .slices
            .into_iter()
            .find(|s| s.slice.id == id)
            .unwrap()
    }

    fn mirror_bytes(&self) -> Vec<u8> {
        fs::read(self.repo.path().join("vsm_data.js")).unwrap()
    }

    fn plan_json(&self) -> PlanFile {
        serde_json::from_str(&self.repo.read(".foundry/plan.json")).unwrap()
    }

    /// Writes the slice's report and a review of it, as the verification feature would.
    fn review(&self, slice_id: &str, passed: bool) -> Value {
        let report = json!({
            "sliceId": slice_id, "agent": "ada", "requirements": [],
            "testsRun": [], "blockers": [], "notes": ""
        });
        self.repo.write(
            &format!(".foundry/reports/{slice_id}.json"),
            &serde_json::to_string_pretty(&report).unwrap(),
        );
        self.repo.write(
            &format!(".foundry/reviews/{slice_id}.json"),
            &json!({
                "sliceId": slice_id, "passed": passed, "findings": [], "testRuns": [],
                "report": report, "verifiedAt": 1
            })
            .to_string(),
        );
        report
    }
}

/// The JSON object inside `const VSM_DATA = …;`.
fn mirror_value(text: &str) -> Value {
    let start = text.find("const VSM_DATA = ").expect("declaration") + "const VSM_DATA = ".len();
    let end = text.rfind(';').expect("trailing semicolon");
    serde_json::from_str(&text[start..end]).unwrap()
}

/// What `vsm_data.js` must contain for this plan.json, built independently of mirror.rs
/// from the field mapping: order = layer + 1, dependsOn → depends_on, decisions → resolved,
/// openQuestions → open, usedBy → used_by, deferred → future_horizon.
fn expected_mirror(file: &PlanFile) -> Value {
    let view = view(file);
    let mut slices: Vec<(u32, String, String, Value)> = view
        .slices
        .iter()
        .map(|s| {
            let mut deps = s.slice.depends_on.clone();
            deps.sort();
            deps.dedup();
            let entry = json!({
                "id": s.slice.id,
                "name": s.slice.name,
                "order": s.layer + 1,
                "status": s.slice.status,
                "depends_on": deps,
                "delivers": s.slice.delivers,
                "used_by": s.used_by,
                "resolved": s.slice.decisions,
                "open": s.slice.open_questions.iter().map(|q| q.text.clone()).collect::<Vec<_>>(),
                "workspace_access": s.slice.workspace_access,
            });
            (s.layer + 1, s.slice.name.clone(), s.slice.id.clone(), entry)
        })
        .collect();
    slices.sort_by(|a, b| (a.0, &a.1, &a.2).cmp(&(b.0, &b.1, &b.2)));
    let mut horizon: Vec<(String, String, Value)> = file
        .deferred
        .iter()
        .map(|d| {
            (
                d.label.clone(),
                d.id.clone(),
                json!({ "id": d.id, "label": d.label, "anchored_to": d.anchored_to }),
            )
        })
        .collect();
    horizon.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    json!({
        "meta": { "title": "Orbit", "subtitle": "Read anywhere" },
        "slices": slices.into_iter().map(|s| s.3).collect::<Vec<_>>(),
        "future_horizon": horizon.into_iter().map(|h| h.2).collect::<Vec<_>>(),
    })
}

// ---------- workspace access ----------

#[test]
fn workspace_access_defaults_off_and_set_follows_the_revision_rules() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    assert!(!env.slice_view(&a).slice.workspace_access, "off by default");
    assert!(env
        .repo
        .read(".foundry/plan.json")
        .contains("\"workspaceAccess\": false"));

    let revision = env.plan().revision;
    let stale = env
        .store
        .set_slice_workspace_access(P, &a, true, Some(revision - 1));
    assert_eq!(stale.unwrap_err().code, ErrorCode::StaleRevision);
    assert!(
        !env.slice_view(&a).slice.workspace_access,
        "a stale set changes nothing"
    );

    let plan = env
        .store
        .set_slice_workspace_access(P, &a, true, Some(revision))
        .unwrap();
    assert_eq!(plan.revision, revision + 1);
    assert!(env.slice_view(&a).slice.workspace_access);
    assert_mirror_matches(&env);
    let mirrored = mirror_value(&String::from_utf8(env.mirror_bytes()).unwrap());
    assert_eq!(mirrored["slices"][0]["workspace_access"], json!(true));

    env.store
        .set_slice_workspace_access(P, &a, false, None)
        .unwrap();
    assert!(!env.slice_view(&a).slice.workspace_access);
    assert_eq!(
        env.store
            .set_slice_workspace_access(P, "nope", true, None)
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[test]
fn a_plan_written_before_workspace_access_still_loads() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    let mut file: Value = serde_json::from_str(&env.repo.read(".foundry/plan.json")).unwrap();
    for slice in file["slices"].as_array_mut().unwrap() {
        slice.as_object_mut().unwrap().remove("workspaceAccess");
    }
    let text = file.to_string();
    assert!(!text.contains("workspaceAccess"));
    env.repo.write(".foundry/plan.json", &text);
    assert!(!env.slice_view(&a).slice.workspace_access);
    // And the next change writes it out.
    env.store
        .set_slice_workspace_access(P, &a, true, None)
        .unwrap();
    assert!(env.slice_view(&a).slice.workspace_access);
}

fn assert_mirror_matches(env: &Env) {
    let text = String::from_utf8(env.mirror_bytes()).unwrap();
    assert!(
        text.starts_with("// vsm_data.js: GENERATED by Foundry"),
        "header"
    );
    assert!(!text.contains('\r'), "LF only");
    assert_eq!(mirror_value(&text), expected_mirror(&env.plan_json()));
}

// ---------- R-2-1 ----------

#[test]
fn a_cycle_is_rejected_and_the_error_names_it() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    let b = env.slice("B", &[&a]);
    let c = env.slice("C", &[&b]);
    let before = env.plan();

    let err = env
        .store
        .set_dependencies(P, &a, vec![c.clone()], None)
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::DependencyCycle);
    assert!(err.message.contains("A → C → B → A"), "{}", err.message);
    assert_eq!(env.plan(), before, "nothing changed, not even the revision");

    let unknown = env.store.set_dependencies(P, &a, vec!["nope".into()], None);
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
    let itself = env.store.set_dependencies(P, &a, vec![a.clone()], None);
    assert_eq!(itself.unwrap_err().code, ErrorCode::InvalidInput);
    let at_add = env.store.add_slice(
        P,
        NewSlice {
            name: "D".into(),
            depends_on: vec!["missing".into()],
            ..Default::default()
        },
        None,
    );
    assert_eq!(at_add.unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(env.plan(), before);
}

// ---------- R-2-2 ----------

#[test]
fn layers_are_correct_for_a_diamond_and_a_parallel_graph() {
    let env = Env::new();
    // Diamond: A → B, A → C, B + C → D.
    let a = env.slice("A", &[]);
    let b = env.slice("B", &[&a]);
    let c = env.slice("C", &[&a]);
    let d = env.slice("D", &[&b, &c]);
    // Parallel: three roots, and a chain beside them.
    let p1 = env.slice("P1", &[]);
    let p2 = env.slice("P2", &[]);
    let p3 = env.slice("P3", &[]);
    let chain = env.slice("Chain", &[&p1]);

    let layer = |id: &str| env.slice_view(id).layer;
    assert_eq!([layer(&a), layer(&b), layer(&c), layer(&d)], [0, 1, 1, 2]);
    assert_eq!(
        [layer(&p1), layer(&p2), layer(&p3), layer(&chain)],
        [0, 0, 0, 1]
    );

    let used_by = |id: &str| env.slice_view(id).used_by;
    assert_eq!(
        [used_by(&a), used_by(&b), used_by(&c), used_by(&d)],
        [2, 1, 1, 0]
    );
    // Ready: queued with every dependency done.
    assert!(env.slice_view(&a).ready && !env.slice_view(&b).ready);

    // A longer branch on one side pushes the join down: longest path, not shortest.
    let b2 = env.slice("B2", &[&b]);
    env.store
        .set_dependencies(P, &d, vec![b2, c], None)
        .unwrap();
    assert_eq!(layer(&d), 3);
}

// ---------- R-2-3 ----------

#[test]
fn an_unassigned_requirement_cannot_be_confirmed() {
    let env = Env::new();
    let r = env.requirement(None);

    let err = env.store.confirm_requirement(P, &r, None).unwrap_err();

    assert_eq!(err.code, ErrorCode::RequirementUnassigned);
    let s = env.slice("A", &[]);
    env.store.assign_requirement(P, &r, Some(s), None).unwrap();
    env.store.confirm_requirement(P, &r, None).unwrap();
    assert_eq!(
        env.plan().requirements[0].state,
        RequirementState::Confirmed
    );

    // Unassigning makes it proposed again.
    env.store.assign_requirement(P, &r, None, None).unwrap();
    assert_eq!(env.plan().requirements[0].state, RequirementState::Proposed);
}

// ---------- R-2-4 ----------

#[test]
fn confirmed_is_false_with_any_unconfirmed_or_unassigned_requirement() {
    let env = Env::new();
    assert!(!env.store.summary(P).unwrap().confirmed, "no slices yet");
    let s = env.slice("A", &[]);
    assert!(
        env.store.summary(P).unwrap().confirmed,
        "a slice, nothing to confirm"
    );

    let assigned = env.requirement(Some(&s));
    let summary = env.store.summary(P).unwrap();
    assert!(!summary.confirmed);
    assert_eq!(summary.unconfirmed, std::slice::from_ref(&assigned));

    env.store.confirm_all(P, None).unwrap();
    assert!(env.store.summary(P).unwrap().confirmed);

    let loose = env.requirement(None);
    let summary = env.store.summary(P).unwrap();
    assert!(!summary.confirmed);
    assert_eq!(summary.unassigned, std::slice::from_ref(&loose));
    env.store.confirm_all(P, None).unwrap();
    assert_eq!(
        env.store.summary(P).unwrap().unconfirmed,
        [loose],
        "confirm_all skips unassigned"
    );

    // A cycle hand-edited into plan.json also means not confirmed, even with no
    // requirements left to confirm.
    let b = env.slice("B", &[&s]);
    let mut file = env.plan_json();
    file.requirements.clear();
    file.slices
        .iter_mut()
        .find(|x| x.id == s)
        .unwrap()
        .depends_on = vec![b];
    fs::write(
        env.repo.path().join(".foundry/plan.json"),
        serde_json::to_string(&file).unwrap(),
    )
    .unwrap();
    let summary = env.store.summary(P).unwrap();
    assert!(!summary.acyclic && !summary.confirmed);
}

// ---------- R-2-5 ----------

#[test]
fn a_slice_with_assigned_requirements_cannot_be_removed() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    let b = env.slice("B", &[]);
    let r = env.requirement(Some(&a));

    let err = env.store.remove_slice(P, &a, None).unwrap_err();
    assert_eq!(err.code, ErrorCode::SliceInUse);
    assert!(err.message.contains("reassign"), "{}", err.message);

    env.store
        .assign_requirement(P, &r, Some(b.clone()), None)
        .unwrap();
    env.store.remove_slice(P, &a, None).unwrap();
    assert_eq!(env.plan().slices.len(), 1);

    // Dependents and anchored deferred items also keep a slice in place.
    let c = env.slice("C", &[&b]);
    assert_eq!(
        env.store.remove_slice(P, &b, None).unwrap_err().code,
        ErrorCode::SliceInUse
    );
    env.store.set_dependencies(P, &c, vec![], None).unwrap();
    let d = env
        .store
        .add_deferred(
            P,
            NewDeferred {
                label: "Later".into(),
                anchored_to: c.clone(),
            },
            None,
        )
        .unwrap()
        .id;
    assert_eq!(
        env.store.remove_slice(P, &c, None).unwrap_err().code,
        ErrorCode::SliceInUse
    );
    env.store.remove_deferred(P, &d, None).unwrap();
    env.store.remove_slice(P, &c, None).unwrap();
}

// ---------- R-2-6 ----------

#[test]
fn the_status_gate_honours_dependencies_and_overrides_need_a_reason() {
    let repo = TestRepo::with_git();
    repo.write("README.md", "hi");
    let env = Env::in_repo(repo);
    let a = env.slice("A", &[]);
    let b = env.slice("B", &[&a]);
    // Setup (plan.json, vsm_data.js) is committed before a slice can start.
    env.repo.git(&["add", "-A"]);
    env.repo.git(&["commit", "--quiet", "-m", "start"]);
    let head = env.repo.git(&["rev-parse", "HEAD"]);
    let start = |id: &str, options: StatusOptions| {
        env.store
            .set_slice_status(P, id, SliceStatus::InProgress, options, None)
    };

    let err = start(&b, StatusOptions::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::DependenciesNotDone);
    assert!(err.message.contains("A"), "{}", err.message);
    for reason in [None, Some("   ".to_string())] {
        let refused = start(
            &b,
            StatusOptions {
                override_gate: true,
                reason,
            },
        )
        .unwrap_err();
        assert_eq!(refused.code, ErrorCode::InvalidInput);
    }
    assert_eq!(env.slice_view(&b).slice.status, SliceStatus::Queued);

    start(
        &b,
        StatusOptions {
            override_gate: true,
            reason: Some("demo on Friday".into()),
        },
    )
    .unwrap();
    let started = env.slice_view(&b).slice;
    assert_eq!(started.status, SliceStatus::InProgress);
    assert_eq!(
        started.decisions,
        ["Started before A was done (override): demo on Friday"]
    );
    assert_eq!(
        started.start_commit.as_deref(),
        Some(head.as_str()),
        "HEAD recorded"
    );

    // Once its dependency is done, a slice starts without an override.
    let c = env.slice("C", &[&a]);
    env.store
        .set_slice_status(
            P,
            &a,
            SliceStatus::InProgress,
            StatusOptions::default(),
            None,
        )
        .unwrap();
    env.review(&a, true);
    env.store
        .set_slice_status(P, &a, SliceStatus::Done, StatusOptions::default(), None)
        .unwrap();
    start(&c, StatusOptions::default()).unwrap();
    assert!(env.slice_view(&c).slice.decisions.is_empty());
}

#[test]
fn a_slice_cant_start_while_setup_files_are_uncommitted() {
    let repo = TestRepo::with_git();
    repo.git(&["commit", "--quiet", "--allow-empty", "-m", "initial"]);
    repo.write("AGENTS.md", "# rules\n");
    repo.write("agents/roles.json", "{}");
    repo.write("docs/brain-dump.md", "ideas");
    let env = Env::in_repo(repo);
    let a = env.slice("A", &[]);
    let start = || {
        env.store.set_slice_status(
            P,
            &a,
            SliceStatus::InProgress,
            StatusOptions::default(),
            None,
        )
    };

    // A fresh project: everything from Setup is untracked.
    let err = start().unwrap_err();
    assert_eq!(err.code, ErrorCode::SetupNotCommitted);
    assert!(
        err.message.starts_with("Commit your setup first"),
        "{}",
        err.message
    );
    for file in [
        "AGENTS.md",
        "agents/roles.json",
        "docs/brain-dump.md",
        ".foundry/plan.json",
    ] {
        assert!(err.message.contains(file), "{file}: {}", err.message);
    }
    assert_eq!(env.slice_view(&a).slice.status, SliceStatus::Queued);
    assert_eq!(env.slice_view(&a).slice.start_commit, None);
    assert_eq!(
        env.repo.git(&["rev-list", "--count", "HEAD"]),
        "1",
        "Foundry never commits for the user"
    );

    // Committed, then edited by hand: still refused.
    env.repo.git(&["add", "-A"]);
    env.repo.git(&["commit", "--quiet", "-m", "setup"]);
    env.repo.write("AGENTS.md", "# edited\n");
    let err = start().unwrap_err();
    assert_eq!(err.code, ErrorCode::SetupNotCommitted);
    assert!(
        err.message.contains("AGENTS.md is not committed"),
        "{}",
        err.message
    );
    env.repo.git(&["checkout", "--", "AGENTS.md"]);

    // Plan edits since the setup commit don't block (Foundry rewrites plan.json and its mirror).
    env.slice("B", &[]);
    start().unwrap();
    let started = env.slice_view(&a).slice;
    assert_eq!(started.status, SliceStatus::InProgress);
    assert_eq!(
        started.start_commit,
        Some(env.repo.git(&["rev-parse", "HEAD"]))
    );
}

// ---------- P9: done needs a passed, current verification ----------

#[test]
fn done_needs_every_verified_report_unchanged_and_no_new_one() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    let report = |agent: &str| {
        json!({
            "sliceId": a, "agent": agent, "requirements": [],
            "testsRun": [], "blockers": [], "notes": ""
        })
    };
    let path = |agent: &str| format!(".foundry/reports/{a}.{agent}.json");
    let write_all = || {
        for agent in ["ada", "qa"] {
            env.repo.write(&path(agent), &report(agent).to_string());
        }
        let _ = std::fs::remove_file(env.repo.path().join(path("zed")));
        env.repo.write(
            &format!(".foundry/reviews/{a}.json"),
            &json!({
                "sliceId": a, "passed": true, "findings": [], "testRuns": [],
                "report": report("ada"), "verifiedAt": 1, "requirements": [],
                "reports": [
                    { "path": path("ada"), "agent": "ada", "report": report("ada") },
                    { "path": path("qa"), "agent": "qa", "report": report("qa") }
                ]
            })
            .to_string(),
        );
    };
    let done = || {
        env.store
            .set_slice_status(P, &a, SliceStatus::Done, StatusOptions::default(), None)
    };
    for change in ["edit qa", "remove qa", "add zed"] {
        write_all();
        match change {
            "edit qa" => {
                env.repo.write(&path("qa"), r#"{ "changed": true }"#);
            }
            "remove qa" => std::fs::remove_file(env.repo.path().join(path("qa"))).unwrap(),
            _ => {
                env.repo.write(&path("zed"), &report("zed").to_string());
            }
        }
        let err = done().unwrap_err();
        assert_eq!(err.code, ErrorCode::NotVerified, "{change}");
        assert!(err.message.contains("stale"), "{change}: {}", err.message);
    }
    write_all();
    done().unwrap();
    assert_eq!(env.slice_view(&a).slice.status, SliceStatus::Done);
}

#[test]
fn done_needs_a_passed_current_verification_or_an_override_with_a_reason() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    let done = |options: StatusOptions| {
        env.store
            .set_slice_status(P, &a, SliceStatus::Done, options, None)
    };
    let refused = |why: &str| {
        let err = done(StatusOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotVerified, "{why}");
        assert!(err.message.contains(why), "{}", err.message);
        assert_eq!(env.slice_view(&a).slice.status, SliceStatus::Queued);
    };

    refused("has no verification");
    env.review(&a, false);
    refused("failed its last verification");
    let report = env.review(&a, true);
    let mut edited = report.clone();
    edited["notes"] = json!("changed after verifying");
    env.repo
        .write(&format!(".foundry/reports/{a}.json"), &edited.to_string());
    refused("stale");
    // A review that doesn't parse counts as none.
    env.repo
        .write(&format!(".foundry/reviews/{a}.json"), "{ nope");
    refused("has no verification");

    // An override needs a reason; a blank one is no reason.
    for reason in [None, Some("   ".to_string())] {
        let err = done(StatusOptions {
            override_gate: true,
            reason,
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }
    // A reason without the override flag doesn't override.
    let err = done(StatusOptions {
        override_gate: false,
        reason: Some("ship it".into()),
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotVerified);

    done(StatusOptions {
        override_gate: true,
        reason: Some("verified by hand".into()),
    })
    .unwrap();
    let slice = env.slice_view(&a).slice;
    assert_eq!(slice.status, SliceStatus::Done);
    assert_eq!(
        slice.decisions,
        ["Marked done without a passed verification (override; it has no verification): verified by hand"]
    );

    // A passed, current verification needs no override, and records nothing.
    let b = env.slice("B", &[]);
    env.review(&b, true);
    env.store
        .set_slice_status(P, &b, SliceStatus::Done, StatusOptions::default(), None)
        .unwrap();
    let slice = env.slice_view(&b).slice;
    assert_eq!(slice.status, SliceStatus::Done);
    assert!(slice.decisions.is_empty());
    // Reformatting the report keeps it current (compared as JSON values).
    let c = env.slice("C", &[]);
    let report = env.review(&c, true);
    env.repo
        .write(&format!(".foundry/reports/{c}.json"), &report.to_string());
    env.store
        .set_slice_status(P, &c, SliceStatus::Done, StatusOptions::default(), None)
        .unwrap();

    // Other moves aren't gated.
    env.store
        .set_slice_status(
            P,
            &a,
            SliceStatus::InProgress,
            StatusOptions::default(),
            None,
        )
        .unwrap();
    env.store
        .set_slice_status(P, &a, SliceStatus::Queued, StatusOptions::default(), None)
        .unwrap();
}

// ---------- R-2-7 ----------

#[test]
fn the_plan_round_trips_through_disk_and_revision_increments() {
    let env = Env::new();
    assert_eq!(env.plan().revision, 0);
    let a = env.slice("A", &[]);
    let q = env
        .store
        .add_open_question(P, &a, "Which DB?", None)
        .unwrap()
        .id;
    env.store
        .resolve_open_question(P, &a, &q, "SQLite", None)
        .unwrap();
    env.requirement(Some(&a));
    assert_eq!(env.plan().revision, 4);
    assert_eq!(
        *env.events.0.lock().unwrap(),
        [1, 2, 3, 4],
        "one event per mutation"
    );

    // A failed mutation changes nothing and emits nothing.
    env.store.remove_slice(P, &a, None).unwrap_err();
    assert_eq!(env.plan().revision, 4);
    assert_eq!(env.events.0.lock().unwrap().len(), 4);

    // A fresh store (a restart) reads the same plan back.
    let reopened = {
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, env.repo.path())]);
        PlanStore::new(repos, Git::system(), Arc::new(Revisions::default()))
    };
    assert_eq!(reopened.get(P).unwrap(), env.plan());
    assert_eq!(env.slice_view(&a).slice.decisions, ["Which DB? → SQLite"]);
}

#[test]
fn an_unreadable_plan_is_refused_not_reset() {
    let env = Env::new();
    env.repo.write(".foundry/plan.json", "{ broken");
    assert_eq!(env.store.get(P).unwrap_err().code, ErrorCode::Internal);
    let err = env.store.add_slice(
        P,
        NewSlice {
            name: "A".into(),
            ..Default::default()
        },
        None,
    );
    assert_eq!(err.unwrap_err().code, ErrorCode::Internal);
    assert_eq!(
        env.repo.read(".foundry/plan.json"),
        "{ broken",
        "left for the user"
    );
}

// ---------- R-2-8 ----------

#[test]
fn a_stale_expected_revision_is_rejected() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    let rename = |rev| {
        env.store.update_slice(
            P,
            &a,
            SlicePatch {
                name: Some("Renamed".into()),
                ..Default::default()
            },
            Some(rev),
        )
    };

    assert_eq!(rename(0).unwrap_err().code, ErrorCode::StaleRevision);
    assert_eq!(env.slice_view(&a).slice.name, "A");
    assert_eq!(rename(1).unwrap().revision, 2);
    assert_eq!(rename(1).unwrap_err().code, ErrorCode::StaleRevision);
}

// ---------- R-2-9 and R-2-12 ----------

#[test]
fn fifty_concurrent_mutations_lose_no_writes_and_the_mirror_is_final() {
    let env = Env::new();
    let hub = env.slice("Hub", &[]);
    let handles: Vec<_> = (0..50)
        .map(|i| {
            let (store, hub) = (Arc::clone(&env.store), hub.clone());
            std::thread::spawn(move || {
                if i % 2 == 0 {
                    store
                        .add_slice(
                            P,
                            NewSlice {
                                name: format!("S{i:02}"),
                                delivers: vec![],
                                depends_on: vec![hub],
                            },
                            None,
                        )
                        .unwrap();
                } else {
                    store
                        .add_decision(P, &hub, &format!("decision {i:02}"), None)
                        .unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let plan = env.plan();
    assert_eq!(plan.revision, 51);
    assert_eq!(plan.slices.len(), 26);
    let hub_view = env.slice_view(&hub);
    assert_eq!(hub_view.slice.decisions.len(), 25);
    assert_eq!(hub_view.used_by, 25);
    let mut revisions = env.events.0.lock().unwrap().clone();
    revisions.sort_unstable();
    assert_eq!(revisions, (1..=51).collect::<Vec<_>>());
    // R-2-12: the mirror is the final state, not an earlier write that landed late.
    assert_mirror_matches(&env);
}

// ---------- R-2-10 ----------

#[test]
fn vsm_data_js_appears_with_the_first_slice() {
    let env = Env::new();
    env.store.get(P).unwrap();
    env.store.summary(P).unwrap();
    assert!(
        !env.repo.path().join("vsm_data.js").exists(),
        "reads never write it"
    );
    env.slice("A", &[]);
    assert!(env.repo.path().join("vsm_data.js").exists());
    assert_mirror_matches(&env);
}

/// A tiny deterministic generator (no rand crate).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n.max(1) as u64).unwrap()
    }

    fn pick<'a>(&mut self, items: &'a [String]) -> Option<&'a String> {
        (!items.is_empty()).then(|| &items[self.below(items.len())])
    }
}

/// Property: over random mutation sequences, vsm_data.js always equals the field mapping of
/// plan.json. Failed mutations (cycles, gates, in-use removals) are part of the mix.
#[test]
fn vsm_data_js_matches_plan_json_over_random_plans() {
    // (slices, dependency edges, deferred, decisions, open questions) seen at the end.
    let mut seen = (0, 0, 0, 0, 0);
    for seed in 1..=12u64 {
        let env = Env::new();
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for step in 0..30 {
            let plan = env.plan();
            let slices: Vec<String> = plan.slices.iter().map(|s| s.slice.id.clone()).collect();
            let deferred: Vec<String> = plan.deferred.iter().map(|d| d.id.clone()).collect();
            let _ = match rng.below(9) {
                0 | 1 => {
                    let deps: Vec<String> = slices
                        .iter()
                        .filter(|_| rng.below(3) == 0)
                        .cloned()
                        .collect();
                    env.store
                        .add_slice(
                            P,
                            NewSlice {
                                name: format!("S{}", rng.below(5)),
                                delivers: vec![format!("d{step}")],
                                depends_on: deps,
                            },
                            None,
                        )
                        .map(|_| ())
                }
                2 => match rng.pick(&slices) {
                    Some(id) => {
                        let deps: Vec<String> = slices
                            .iter()
                            .filter(|_| rng.below(3) == 0)
                            .cloned()
                            .collect();
                        env.store.set_dependencies(P, id, deps, None).map(|_| ())
                    }
                    None => Ok(()),
                },
                3 => match rng.pick(&slices) {
                    Some(id) => {
                        let status = [
                            SliceStatus::Queued,
                            SliceStatus::InProgress,
                            SliceStatus::Done,
                        ][rng.below(3)];
                        let options = StatusOptions {
                            override_gate: rng.below(2) == 0,
                            reason: Some("why not".into()),
                        };
                        env.store
                            .set_slice_status(P, id, status, options, None)
                            .map(|_| ())
                    }
                    None => Ok(()),
                },
                4 => match rng.pick(&slices) {
                    Some(id) => env
                        .store
                        .add_decision(P, id, &format!("dec {step}"), None)
                        .map(|_| ()),
                    None => Ok(()),
                },
                5 => match rng.pick(&slices) {
                    Some(id) => env
                        .store
                        .add_open_question(P, id, &format!("q {step}?"), None)
                        .map(|_| ()),
                    None => Ok(()),
                },
                6 => match rng.pick(&slices) {
                    Some(id) => env
                        .store
                        .add_deferred(
                            P,
                            NewDeferred {
                                label: format!("later {}", rng.below(4)),
                                anchored_to: id.clone(),
                            },
                            None,
                        )
                        .map(|_| ()),
                    None => Ok(()),
                },
                7 => match rng.pick(&deferred) {
                    Some(id) => env.store.remove_deferred(P, id, None).map(|_| ()),
                    None => Ok(()),
                },
                _ => match rng.pick(&slices) {
                    Some(id) => env.store.remove_slice(P, id, None).map(|_| ()),
                    None => Ok(()),
                },
            };
            if env.plan().revision > 0 {
                assert_mirror_matches(&env);
            }
        }
        let plan = env.plan();
        seen.0 += plan.slices.len();
        seen.1 += plan
            .slices
            .iter()
            .map(|s| s.slice.depends_on.len())
            .sum::<usize>();
        seen.2 += plan.deferred.len();
        seen.3 += plan
            .slices
            .iter()
            .map(|s| s.slice.decisions.len())
            .sum::<usize>();
        seen.4 += plan
            .slices
            .iter()
            .map(|s| s.slice.open_questions.len())
            .sum::<usize>();
    }
    // The generator really builds varied plans (not 12 empty ones).
    let (slices, edges, deferred, decisions, questions) = seen;
    assert!(slices >= 20 && edges >= 5 && deferred >= 3, "{seen:?}");
    assert!(decisions >= 5 && questions >= 3, "{seen:?}");
}

// ---------- R-2-11 ----------

#[test]
fn vsm_data_js_is_byte_identical_for_two_paths_to_the_same_state() {
    let first = Env::new();
    let a = first.slice("A", &[]);
    let b = first.slice("B", &[]);
    first
        .store
        .add_deferred(
            P,
            NewDeferred {
                label: "Sync".into(),
                anchored_to: a.clone(),
            },
            None,
        )
        .unwrap();
    // A second repo starting from the very same plan.json (same ids).
    let second = Env::new();
    fs::create_dir_all(second.repo.path().join(".foundry")).unwrap();
    fs::copy(
        first.repo.path().join(".foundry/plan.json"),
        second.repo.path().join(".foundry/plan.json"),
    )
    .unwrap();

    // Path 1: one step.
    first
        .store
        .set_dependencies(P, &b, vec![a.clone()], None)
        .unwrap();
    // Path 2: a detour that ends in the same state.
    let rename = |name: &str| SlicePatch {
        name: Some(name.into()),
        ..Default::default()
    };
    second
        .store
        .update_slice(P, &a, rename("Temp"), None)
        .unwrap();
    second
        .store
        .set_dependencies(P, &a, vec![b.clone()], None)
        .unwrap();
    second.store.set_dependencies(P, &a, vec![], None).unwrap();
    let extra = second
        .store
        .add_deferred(
            P,
            NewDeferred {
                label: "Aaa".into(),
                anchored_to: b.clone(),
            },
            None,
        )
        .unwrap()
        .id;
    second
        .store
        .set_dependencies(P, &b, vec![a.clone()], None)
        .unwrap();
    second.store.remove_deferred(P, &extra, None).unwrap();
    second.store.update_slice(P, &a, rename("A"), None).unwrap();

    assert_ne!(
        first.plan().revision,
        second.plan().revision,
        "different histories"
    );
    assert_eq!(first.mirror_bytes(), second.mirror_bytes());
}

// ---------- mirror details ----------

#[test]
fn a_failed_mirror_write_restores_plan_json() {
    let env = Env::new();
    env.slice("A", &[]);
    let before = env.repo.read(".foundry/plan.json");
    // Make vsm_data.js unwritable: a folder in its place.
    fs::remove_file(env.repo.path().join("vsm_data.js")).unwrap();
    fs::create_dir(env.repo.path().join("vsm_data.js")).unwrap();

    let err = env.store.add_slice(
        P,
        NewSlice {
            name: "B".into(),
            ..Default::default()
        },
        None,
    );

    assert!(err.is_err());
    assert_eq!(
        env.repo.read(".foundry/plan.json"),
        before,
        "plan.json put back"
    );
    assert_eq!(env.plan().slices.len(), 1);
    assert_eq!(
        env.events.0.lock().unwrap().len(),
        1,
        "no event for the failed change"
    );
}

#[test]
fn hand_edits_to_vsm_data_js_are_overwritten_by_the_next_change() {
    let env = Env::new();
    let a = env.slice("A", &[]);
    fs::write(env.repo.path().join("vsm_data.js"), "// my notes").unwrap();
    env.store
        .add_decision(P, &a, "keep it simple", None)
        .unwrap();
    assert_mirror_matches(&env);
}

#[test]
fn requirements_are_checked_against_the_brain_dump() {
    let env = Env::new();
    env.repo
        .write("docs/brain-dump.md", "Users can save\na reading list.");
    let add = |excerpt: &str| {
        env.store
            .add_requirement(
                P,
                NewRequirement {
                    text: "Save".into(),
                    kind: RequirementKind::Behavior,
                    slice_id: None,
                    source_excerpt: excerpt.into(),
                    verified_by: String::new(),
                },
                None,
            )
            .unwrap()
            .plan
            .requirements
            .last()
            .unwrap()
            .unsourced
    };
    assert!(!add("save a   reading list"), "whitespace-normalized match");
    assert!(add("share lists"), "not in the brain dump");
    assert!(add(""), "an empty excerpt is no source");
}

#[test]
fn editing_a_confirmed_requirement_makes_it_proposed_again() {
    let env = Env::new();
    let s = env.slice("A", &[]);
    let r = env.requirement(Some(&s));
    env.store.confirm_requirement(P, &r, None).unwrap();
    let patch = |p: RequirementPatch| env.store.update_requirement(P, &r, p, None).unwrap();

    let same = patch(RequirementPatch {
        verified_by: Some("another test".into()),
        ..Default::default()
    });
    assert_eq!(
        same.requirements[0].state,
        RequirementState::Confirmed,
        "verifiedBy only"
    );
    let changed = patch(RequirementPatch {
        text: Some("It works well".into()),
        ..Default::default()
    });
    assert_eq!(changed.requirements[0].state, RequirementState::Proposed);
}

#[test]
fn the_template_documents_the_same_schema_as_the_mirror() {
    let template = include_str!("../../../resources/templates/vsm_data.template.js");
    let value = mirror_value(template);
    assert_eq!(
        value,
        json!({ "meta": { "title": "", "subtitle": "" }, "slices": [], "future_horizon": [] })
    );
    let env = Env::new();
    env.slice("A", &[]);
    let generated = String::from_utf8(env.mirror_bytes()).unwrap();
    // Same schema lines in both headers.
    let schema = |text: &str| -> Vec<String> {
        text.lines()
            .skip_while(|l| !l.starts_with("// Schema"))
            .take_while(|l| l.starts_with("//"))
            .map(str::to_string)
            .collect()
    };
    assert_eq!(schema(template), schema(&generated));
    assert!(!schema(template).is_empty());
}

#[test]
fn commands_round_trip_over_ipc() {
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::{Manager, WebviewWindowBuilder};

    let env = Env::new();
    let app = crate::app_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Arc::clone(&env.store));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, body: Value| {
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
        .map(|body| body.deserialize::<Value>().unwrap())
    };

    let a = invoke(
        "add_slice",
        json!({ "projectId": P, "slice": { "name": "A", "delivers": [], "dependsOn": [] }, "expectedRevision": 0 }),
    )
    .unwrap();
    let a_id = a["id"].as_str().unwrap().to_string();
    let b = invoke(
        "add_slice",
        json!({ "projectId": P, "slice": { "name": "B", "delivers": [], "dependsOn": [a_id] }, "expectedRevision": null }),
    )
    .unwrap();
    let b_id = b["id"].as_str().unwrap().to_string();
    assert_eq!(b["plan"]["revision"], 2);
    assert_eq!(b["plan"]["slices"][1]["layer"], 1);
    assert_eq!(b["plan"]["slices"][1]["dependsOn"][0], a_id.as_str());

    let stale = invoke(
        "update_slice",
        json!({ "projectId": P, "sliceId": a_id, "patch": { "name": "X", "delivers": null }, "expectedRevision": 0 }),
    )
    .unwrap_err();
    assert_eq!(stale["code"], "stale_revision");

    let gated = invoke(
        "set_slice_status",
        json!({ "projectId": P, "sliceId": b_id, "status": "in_progress",
                "options": { "override": false, "reason": null }, "expectedRevision": null }),
    )
    .unwrap_err();
    assert_eq!(gated["code"], "dependencies_not_done");
    let overridden = invoke(
        "set_slice_status",
        json!({ "projectId": P, "sliceId": b_id, "status": "in_progress",
                "options": { "override": true, "reason": "deadline" }, "expectedRevision": null }),
    )
    .unwrap();
    assert_eq!(overridden["slices"][1]["status"], "in_progress");
    assert!(overridden["slices"][1]["decisions"][0]
        .as_str()
        .unwrap()
        .ends_with("(override): deadline"));

    let summary = invoke("plan_summary", json!({ "projectId": P })).unwrap();
    assert_eq!(summary["confirmed"], true);
    assert_eq!(
        summary["ready"],
        json!([a_id]),
        "A: queued, no dependencies"
    );
    assert_eq!(
        summary["blocked"],
        json!([]),
        "B is in progress, so neither ready nor blocked"
    );
}

// ---------- loading a generated roadmap (P3's plan half) ----------

/// `.foundry/roadmap.json` as roadmap generation writes it.
fn roadmap_json() -> Value {
    json!({
        "roadmap": {
            "goal": "Read anywhere",
            "targetUsers": [], "featureMap": [],
            "slices": [
                { "name": "Core", "delivers": ["save a list"], "dependsOn": [] },
                { "name": "Offline", "delivers": ["read offline"], "dependsOn": ["Core"] }
            ],
            "requirements": [
                { "text": "Save a reading list", "kind": "behavior", "sourceExcerpt": "save a reading list",
                  "sliceName": "Core", "verifiedBy": "add one", "unsourced": false },
                { "text": "Share lists", "kind": "interaction", "sourceExcerpt": "made up",
                  "sliceName": "Offline", "verifiedBy": "share one", "unsourced": true }
            ],
            "deferred": [
                { "label": "Sync later", "anchoredTo": "Offline" },
                { "label": "Social", "anchoredTo": "Library family" }
            ],
            "openQuestions": ["What does it cost?", "  "]
        },
        "unassigned": [
            { "text": "Works for a week", "kind": "constraint", "sourceExcerpt": "a week",
              "sliceName": null, "verifiedBy": "wait", "unsourced": false }
        ],
        "uncoveredParagraphs": [], "attempts": 1, "generatedAt": 1
    })
}

fn write_roadmap(env: &Env, value: &Value) {
    env.repo.write(".foundry/roadmap.json", &value.to_string());
}

fn load(env: &Env, discard: bool) -> Result<RoadmapLoaded, AppError> {
    env.store.load_roadmap(
        P,
        LoadRoadmapOptions {
            discard_confirmed: discard,
        },
        None,
    )
}

/// Bytes of plan.json and vsm_data.js (None when absent or a folder), and the event count.
fn plan_state(env: &Env) -> (Option<Vec<u8>>, Option<Vec<u8>>, usize) {
    (
        fs::read(env.repo.path().join(".foundry/plan.json")).ok(),
        fs::read(env.repo.path().join("vsm_data.js")).ok(),
        env.events.0.lock().unwrap().len(),
    )
}

// R-3-1
#[test]
fn a_generated_roadmap_lands_in_the_plan_as_proposed_in_one_mutation() {
    let env = Env::new();
    let old = env.slice("Old slice", &[]);
    write_roadmap(&env, &roadmap_json());

    let loaded = load(&env, false).unwrap();

    let plan = &loaded.plan;
    assert_eq!(plan.revision, 2, "exactly one revision for the whole load");
    assert_eq!(
        *env.events.0.lock().unwrap(),
        [1, 2],
        "exactly one plan:changed"
    );
    assert!(
        !plan.slices.iter().any(|s| s.slice.id == old),
        "replaced, not merged"
    );
    let names: Vec<&str> = plan.slices.iter().map(|s| s.slice.name.as_str()).collect();
    assert_eq!(names, ["Core", "Offline"]);
    let core = &plan.slices[0];
    let offline = &plan.slices[1];
    assert_eq!(
        offline.slice.depends_on,
        std::slice::from_ref(&core.slice.id),
        "names became ids"
    );
    assert_eq!(offline.layer, 1);
    assert!(plan
        .slices
        .iter()
        .all(|s| s.slice.status == SliceStatus::Queued));

    assert_eq!(plan.requirements.len(), 3, "assigned + unassigned");
    assert!(plan
        .requirements
        .iter()
        .all(|r| r.state == RequirementState::Proposed));
    let by_text = |t: &str| plan.requirements.iter().find(|r| r.text == t).unwrap();
    assert_eq!(
        by_text("Save a reading list").slice_id.as_ref(),
        Some(&core.slice.id)
    );
    assert_eq!(
        by_text("Save a reading list").kind,
        RequirementKind::Behavior
    );
    assert!(
        by_text("Share lists").unsourced,
        "unsourced flag carried over"
    );
    assert!(!by_text("Save a reading list").unsourced);
    assert_eq!(
        by_text("Works for a week").slice_id,
        None,
        "unassigned stays unassigned"
    );

    assert_eq!(plan.deferred.len(), 1);
    assert_eq!(plan.deferred[0].anchored_to, offline.slice.id);
    assert_eq!(
        loaded.unanchored_deferred,
        ["Social"],
        "reported, not silently dropped"
    );
    let questions: Vec<&str> = plan
        .open_questions
        .iter()
        .map(|q| q.text.as_str())
        .collect();
    assert_eq!(questions, ["What does it cost?"]);
    assert_eq!(
        (
            loaded.slices,
            loaded.requirements,
            loaded.deferred,
            loaded.open_questions
        ),
        (2, 3, 1, 1)
    );
    assert_eq!(env.plan(), *plan, "what was returned is what's on disk");
    assert_mirror_matches(&env);
}

// R-3-8
#[test]
fn loading_is_refused_over_confirmed_requirements_unless_discard_confirmed() {
    let env = Env::new();
    let s = env.slice("A", &[]);
    let r = env.requirement(Some(&s));
    env.store.confirm_requirement(P, &r, None).unwrap();
    write_roadmap(&env, &roadmap_json());
    let before = plan_state(&env);

    let err = load(&env, false).unwrap_err();

    assert_eq!(err.code, ErrorCode::ConfirmedRequirements);
    assert!(err.message.contains("1 confirmed"), "{}", err.message);
    assert_eq!(
        plan_state(&env),
        before,
        "plan.json, vsm_data.js and events untouched"
    );

    let loaded = load(&env, true).unwrap();
    assert_eq!(loaded.plan.requirements.len(), 3);
    assert!(loaded
        .plan
        .requirements
        .iter()
        .all(|r| r.state == RequirementState::Proposed));
}

/// Any problem anywhere in the roadmap fails the whole load: plan.json, vsm_data.js and the
/// event stream are exactly as before.
#[test]
fn a_load_that_fails_midway_changes_nothing() {
    let env = Env::new();
    env.slice("A", &[]);
    let base = roadmap_json();
    let mut bad_kind = base.clone();
    bad_kind["unassigned"][0]["kind"] = json!("functional"); // the very last item
    let mut unknown_dep = base.clone();
    unknown_dep["roadmap"]["slices"][1]["dependsOn"] = json!(["Core", "Ghost"]);
    let mut cycle = base.clone();
    cycle["roadmap"]["slices"][0]["dependsOn"] = json!(["Offline"]);
    let mut duplicate = base.clone();
    duplicate["roadmap"]["slices"][1]["name"] = json!("Core");

    let cases = [
        (
            "bad kind",
            Some(bad_kind.to_string()),
            ErrorCode::InvalidInput,
        ),
        (
            "unknown dependency",
            Some(unknown_dep.to_string()),
            ErrorCode::InvalidInput,
        ),
        ("cycle", Some(cycle.to_string()), ErrorCode::DependencyCycle),
        (
            "duplicate slice",
            Some(duplicate.to_string()),
            ErrorCode::InvalidInput,
        ),
        (
            "unreadable",
            Some("{ nope".to_string()),
            ErrorCode::InvalidInput,
        ),
        ("missing", None, ErrorCode::NotFound),
    ];
    for (case, roadmap, code) in cases {
        let path = env.repo.path().join(".foundry/roadmap.json");
        match &roadmap {
            Some(text) => fs::write(&path, text).unwrap(),
            None => {
                let _ = fs::remove_file(&path);
            }
        }
        let before = plan_state(&env);
        let err = load(&env, true).unwrap_err();
        assert_eq!(err.code, code, "{case}: {}", err.message);
        assert_eq!(plan_state(&env), before, "{case}: something changed");
    }

    // The last step failing (vsm_data.js can't be written) also leaves both files as they
    // were: plan.json is restored.
    write_roadmap(&env, &base);
    let before = plan_state(&env);
    fs::remove_file(env.repo.path().join("vsm_data.js")).unwrap();
    fs::create_dir(env.repo.path().join("vsm_data.js")).unwrap();
    assert!(load(&env, true).is_err());
    assert_eq!(
        fs::read(env.repo.path().join(".foundry/plan.json")).ok(),
        before.0
    );
    assert!(
        env.repo.path().join("vsm_data.js").is_dir(),
        "mirror untouched"
    );
    assert_eq!(env.events.0.lock().unwrap().len(), before.2, "no event");
}

#[test]
fn load_roadmap_honours_expected_revision() {
    let env = Env::new();
    env.slice("A", &[]);
    write_roadmap(&env, &roadmap_json());
    let stale = env
        .store
        .load_roadmap(P, LoadRoadmapOptions::default(), Some(0));
    assert_eq!(stale.unwrap_err().code, ErrorCode::StaleRevision);
    let fresh = env
        .store
        .load_roadmap(P, LoadRoadmapOptions::default(), Some(1));
    assert_eq!(fresh.unwrap().plan.revision, 2);
}
