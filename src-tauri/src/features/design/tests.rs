use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tempfile::TempDir;

use super::*;
use crate::testing::TestRepo;

const P: &str = "project-1";
const REAL_TEMPLATE: &str = include_str!("../../../resources/templates/AGENTS.template.md");

// ---------- fixtures ----------

struct Env {
    repo: TestRepo,
    data: TempDir,
    sources: TempDir,
    template_id: Arc<Mutex<String>>,
    store: DesignStore,
}

impl Env {
    fn new() -> Self {
        Self::in_repo(TestRepo::new())
    }

    fn in_repo(repo: TestRepo) -> Self {
        let data = tempfile::tempdir().unwrap();
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let template_id = Arc::new(Mutex::new("default".to_string()));
        let id = Arc::clone(&template_id);
        let store = DesignStore::new(data.path(), repos, move || id.lock().unwrap().clone());
        Self {
            repo,
            data,
            sources: tempfile::tempdir().unwrap(),
            template_id,
            store,
        }
    }

    /// Adds a library file with these bytes and tags; returns its id.
    fn library_file(&self, name: &str, bytes: &[u8], families: &[&str]) -> String {
        let path = self.sources.path().join(name);
        fs::write(&path, bytes).unwrap();
        let library = self
            .store
            .add_from_paths(&[path.to_string_lossy().into_owned()])
            .unwrap();
        let id = library
            .files
            .iter()
            .find(|f| f.filename == name)
            .unwrap()
            .id
            .clone();
        self.store
            .update(
                &id,
                DesignFilePatch {
                    families: families.iter().map(|f| f.to_string()).collect(),
                    description: format!("About {name}"),
                },
            )
            .unwrap();
        id
    }

    /// Confirms every placeholder: markers get "<label> value" (optional ones ""), and
    /// designFamily gets `families`.
    fn confirm_all(&self, families: &str) {
        for p in self.store.placeholders(P).unwrap() {
            let value = if p.key == DESIGN_FAMILY_KEY {
                families.to_string()
            } else if p.optional {
                String::new()
            } else {
                format!("{} value", p.label)
            };
            self.store.confirm_placeholder(P, &p.key, &value).unwrap();
        }
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        fs::read(self.repo.path().join(rel)).ok()
    }

    fn generate(&self, overwrite: bool) -> Result<GenerateAgentsResult, AppError> {
        self.store.generate(P, GenerateOptions { overwrite })
    }
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Markers in the real template, counted independently of the parser: every `[[ FILL IN`
/// outside the usage blockquote (lines starting with `>`).
fn markers_in_real_template() -> usize {
    REAL_TEMPLATE
        .replace("\r\n", "\n")
        .lines()
        .filter(|l| !l.starts_with('>'))
        .map(|l| l.matches("[[ FILL IN").count())
        .sum()
}

// ---------- R-4-1 ----------

#[test]
fn template_placeholders_equal_the_markers_in_the_real_template() {
    let env = Env::new();
    let placeholders = env.store.placeholders(P).unwrap();
    let in_template: Vec<&Placeholder> = placeholders.iter().filter(|p| p.in_template).collect();

    assert_eq!(in_template.len(), markers_in_real_template());
    assert_eq!(
        in_template.len(),
        7,
        "the template has 7 real markers today"
    );
    let keys: Vec<&str> = in_template.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "architecture",
            "frontend-engine",
            "ui-component-library",
            "backend-engine",
            "database-layer",
            "project-specific-correctness-rule",
            "cross-slice-integration-check"
        ]
    );
    // The marker that wraps onto a second line is one placeholder with its whole hint.
    let ui = in_template
        .iter()
        .find(|p| p.key == "ui-component-library")
        .unwrap();
    assert!(ui.hint.contains("shipped without it"), "{}", ui.hint);
    assert!(in_template.iter().filter(|p| p.optional).count() == 1);

    let family = placeholders
        .iter()
        .find(|p| p.key == DESIGN_FAMILY_KEY)
        .unwrap();
    assert!(!family.in_template, "designFamily isn't a template marker");
    assert_eq!(placeholders.len(), 8);
}

#[test]
fn design_family_suggestions_come_from_the_library() {
    let env = Env::new();
    env.library_file("a.md", b"a", &["SaaS-Dashboard", " marketing "]);
    env.library_file("b.md", b"b", &["playful-editor"]);
    let family = env
        .store
        .placeholders(P)
        .unwrap()
        .into_iter()
        .find(|p| p.key == DESIGN_FAMILY_KEY)
        .unwrap();
    assert_eq!(
        family.suggestions,
        ["marketing", "playful-editor", "saas-dashboard"]
    );
}

#[test]
fn confirm_placeholder_validates() {
    let env = Env::new();
    assert_eq!(
        env.store
            .confirm_placeholder(P, "nope", "x")
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        env.store
            .confirm_placeholder(P, "architecture", "  ")
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        env.store
            .confirm_placeholder(P, "architecture", "[[ FILL IN: sneaky ]]")
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    let list = env
        .store
        .confirm_placeholder(P, DESIGN_FAMILY_KEY, " Marketing, saas ,marketing")
        .unwrap();
    let family = list.iter().find(|p| p.key == DESIGN_FAMILY_KEY).unwrap();
    assert_eq!(family.value.as_deref(), Some("marketing, saas"));
    assert!(family.confirmed);
}

// ---------- R-4-2 ----------

#[test]
fn generate_is_refused_with_an_unconfirmed_placeholder() {
    let env = Env::new();
    env.confirm_all("");
    // Unconfirm one by pointing at a fresh repo state: drop it from the confirmed file.
    let path = env.repo.path().join(PLACEHOLDERS_FILE);
    let mut confirmed: Confirmed =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    confirmed.values.remove("database-layer");
    fs::write(&path, serde_json::to_string(&confirmed).unwrap()).unwrap();
    let before = snapshot(env.repo.path());

    let err = env.generate(false).unwrap_err();

    assert_eq!(err.code, ErrorCode::PlaceholdersUnconfirmed);
    assert!(err.message.contains("Database Layer"), "{}", err.message);
    assert_eq!(snapshot(env.repo.path()), before, "nothing written");

    // designFamily counts too.
    let fresh = Env::new();
    for p in fresh
        .store
        .placeholders(P)
        .unwrap()
        .into_iter()
        .filter(|p| p.in_template)
    {
        let value = if p.optional {
            String::new()
        } else {
            "x".into()
        };
        fresh.store.confirm_placeholder(P, &p.key, &value).unwrap();
    }
    let err = fresh.generate(false).unwrap_err();
    assert!(err.message.contains("Design family"), "{}", err.message);
}

// ---------- R-4-3 ----------

#[test]
fn generated_agents_md_has_no_fill_in_markers_and_no_template_furniture() {
    let env = Env::new();
    env.confirm_all("");

    let result = env.generate(false).unwrap();

    assert!(result.agents_md_written);
    let text = String::from_utf8(env.read("AGENTS.md").unwrap()).unwrap();
    assert!(!text.contains("[[ FILL IN"), "{text}");
    assert!(text.starts_with("### 📜 AGENTS.md - MASTER ARCHITECTURAL PROMPT CONTRACT\n\n### 1."));
    assert!(
        !text.contains("(TEMPLATE)") && !text.contains("> Usage:"),
        "usage note stripped"
    );
    assert!(text.contains("- Architecture: Architecture value\n"));
    // The two-line marker became one filled line.
    assert!(text.contains("- UI Component Library: UI Component Library value\n- Backend Engine:"));
    // The optional bullet, confirmed empty, is gone with its line.
    assert!(!text.contains("Project-Specific Correctness Rule"));
    assert!(text.contains("* **Deterministic Mocking Rules:**"));
    assert!(text.contains("exercises Cross-Slice Integration Check value."));
    assert!(!text.contains('\r'), "LF");
    // Section 6 routes through the manifest and ranks the Bakery between tokens and guides.
    let tokens = text.find("3. The project's own token file").unwrap();
    let bakery = text.find("4. The Bakery").unwrap();
    let guides = text.find("5. The design guideline files").unwrap();
    assert!(tokens < bakery && bakery < guides);
    assert!(text.contains("Read `docs/design/manifest.md`"));
}

// ---------- R-4-4 ----------

#[test]
fn a_differing_agents_md_is_kept_unless_overwrite_and_backed_up_when_replaced() {
    let env = Env::new();
    env.confirm_all("");
    env.repo.write("AGENTS.md", "# my own rules\n");

    let err = env.generate(false).unwrap_err();
    assert_eq!(err.code, ErrorCode::AgentsMdExists);
    assert_eq!(env.read("AGENTS.md").unwrap(), b"# my own rules\n");
    assert!(!env.repo.path().join(".foundry/backups").exists());

    let result = env.generate(true).unwrap();
    let backup = result.backup.unwrap();
    assert!(backup.starts_with(".foundry/backups/AGENTS.") && backup.ends_with(".md"));
    assert_eq!(env.read(&backup).unwrap(), b"# my own rules\n");
    assert!(String::from_utf8(env.read("AGENTS.md").unwrap())
        .unwrap()
        .starts_with("### 📜"));

    // Already identical: nothing to replace, no backup, not an error.
    let again = env.generate(false).unwrap();
    assert!(!again.agents_md_written && again.backup.is_none());
}

// ---------- R-4-5, R-4-7, R-4-8 ----------

#[test]
fn only_intersecting_tagged_files_are_copied_byte_identical_and_the_manifest_lists_them() {
    let env = Env::new();
    let a = "# Dashboards\r\n\r\nUse one accent. — ✓\n".as_bytes();
    let c = b"\xEF\xBB\xBF# Playful\n\tTabs and a BOM\n";
    env.library_file("a.md", a, &["saas-dashboard"]);
    env.library_file("b.md", b"# Marketing\n", &["marketing"]);
    env.library_file("c.md", c, &["saas-dashboard", "playful-editor"]);
    env.library_file("untagged.md", b"# Nobody's\n", &[]);
    assert_eq!(env.store.library().unwrap().untagged_count, 1);
    env.confirm_all("saas-dashboard");

    let result = env.generate(false).unwrap();

    assert_eq!(result.copied, ["a.md", "c.md"]);
    assert!(!result.design_copy_skipped);
    assert_eq!(env.read("docs/design/a.md").unwrap(), a, "byte-identical");
    assert_eq!(env.read("docs/design/c.md").unwrap(), c, "byte-identical");
    assert!(env.read("docs/design/b.md").is_none());
    assert!(env.read("docs/design/untagged.md").is_none());

    let manifest = String::from_utf8(env.read("docs/design/manifest.md").unwrap()).unwrap();
    let listed: Vec<&str> = manifest.lines().filter(|l| l.starts_with("- `")).collect();
    assert_eq!(
        listed,
        [
            "- `a.md` (families: saas-dashboard): About a.md",
            "- `c.md` (families: playful-editor, saas-dashboard): About c.md"
        ]
    );
    assert!(manifest.contains("Project design family: saas-dashboard"));
}

#[test]
fn an_untagged_file_is_never_copied_into_any_project() {
    let env = Env::new();
    env.library_file("untagged.md", b"# Nobody's\n", &[]);
    env.library_file("tagged.md", b"# Tagged\n", &["x"]);
    // Every tag the library knows, and then some.
    env.confirm_all("x, y, z, untagged, none");
    let result = env.generate(false).unwrap();
    assert_eq!(result.copied, ["tagged.md"]);
    assert!(env.read("docs/design/untagged.md").is_none());
}

// ---------- R-4-6 ----------

#[test]
fn a_family_with_no_matching_file_skips_the_copy_and_is_reported() {
    let env = Env::new();
    env.library_file("a.md", b"a", &["saas-dashboard"]);
    env.confirm_all("micro-utility");

    let result = env.generate(false).unwrap();

    assert!(result.design_copy_skipped);
    assert!(result.copied.is_empty());
    assert_eq!(result.unmatched_families, ["micro-utility"]);
    assert!(env.read("AGENTS.md").is_some(), "AGENTS.md still written");
    let manifest = String::from_utf8(env.read("docs/design/manifest.md").unwrap()).unwrap();
    assert!(manifest.contains("No Design Library file matches"));
    assert!(!manifest.lines().any(|l| l.starts_with("- `")));
}

#[test]
fn copies_that_stop_matching_are_removed_next_time() {
    let env = Env::new();
    env.library_file("a.md", b"a", &["one"]);
    env.library_file("b.md", b"b", &["two"]);
    env.confirm_all("one");
    env.generate(false).unwrap();
    env.repo.write("docs/design/mine.md", "the user's own file");

    env.store
        .confirm_placeholder(P, DESIGN_FAMILY_KEY, "two")
        .unwrap();
    let result = env.generate(false).unwrap();

    assert_eq!(result.copied, ["b.md"]);
    assert_eq!(result.removed, ["a.md"]);
    assert!(env.read("docs/design/a.md").is_none());
    assert_eq!(
        env.read("docs/design/mine.md").unwrap(),
        b"the user's own file",
        "not ours"
    );
}

// ---------- R-4-9 ----------

#[test]
fn two_generations_from_the_same_inputs_are_byte_identical() {
    let build = || {
        let env = Env::new();
        env.library_file("a.md", b"# A\r\n", &["fam"]);
        env.library_file("b.md", b"# B\n", &["fam", "other"]);
        env.confirm_all("fam");
        env.generate(false).unwrap();
        env
    };
    let (one, two) = (build(), build());
    let files = [
        "AGENTS.md",
        "docs/design/a.md",
        "docs/design/b.md",
        "docs/design/manifest.md",
    ];
    for file in files {
        assert_eq!(one.read(file), two.read(file), "{file}");
    }
    // And again in the same repo.
    let before: Vec<_> = files.iter().map(|f| one.read(f)).collect();
    let again = one.generate(false).unwrap();
    assert!(!again.agents_md_written);
    assert_eq!(
        files.iter().map(|f| one.read(f)).collect::<Vec<_>>(),
        before
    );
}

// ---------- R-4-17 (design part) ----------

#[test]
fn generate_writes_only_inside_the_allowlist() {
    let env = Env::new();
    env.repo.write("src/main.rs", "fn main() {}");
    env.library_file("a.md", b"a", &["fam"]);
    let before = snapshot(env.repo.path());
    env.confirm_all("fam");
    env.generate(false).unwrap();

    for (path, bytes) in snapshot(env.repo.path()) {
        if before.get(&path) != Some(&bytes) {
            assert!(
                repo_write::is_allowed(Path::new(&path)),
                "{path} is outside the allowlist"
            );
        }
    }
}

// ---------- library ----------

#[test]
fn the_library_takes_md_files_only_all_or_nothing() {
    let env = Env::new();
    let good = env.sources.path().join("good.md");
    let bad = env.sources.path().join("notes.txt");
    fs::write(&good, "ok").unwrap();
    fs::write(&bad, "no").unwrap();
    let both = [good.clone(), bad].map(|p| p.to_string_lossy().into_owned());

    let err = env.store.add_from_paths(&both).unwrap_err();

    assert_eq!(err.code, ErrorCode::UnsupportedFile);
    assert!(env.store.library().unwrap().files.is_empty());
    assert!(!env.data.path().join(LIBRARY_DIR).join("good.md").exists());

    let upper = env.sources.path().join("UPPER.MD");
    fs::write(&upper, "ok").unwrap();
    let added = env
        .store
        .add_from_paths(&[
            good.to_string_lossy().into_owned(),
            upper.to_string_lossy().into_owned(),
        ])
        .unwrap();
    assert_eq!(added.files.len(), 2);
    assert_eq!(added.untagged_count, 2);
    let dup = env
        .store
        .add_from_paths(&[good.to_string_lossy().into_owned()])
        .unwrap_err();
    assert_eq!(dup.code, ErrorCode::InvalidInput, "filenames are unique");
    let manifest = env.sources.path().join("manifest.md");
    fs::write(&manifest, "x").unwrap();
    let reserved = env
        .store
        .add_from_paths(&[manifest.to_string_lossy().into_owned()]);
    assert_eq!(reserved.unwrap_err().code, ErrorCode::UnsupportedFile);
}

#[test]
fn removing_a_library_file_deletes_it() {
    let env = Env::new();
    let id = env.library_file("a.md", b"a", &["x"]);
    assert!(env.data.path().join(LIBRARY_DIR).join("a.md").exists());
    let library = env.store.remove(&id).unwrap();
    assert!(library.files.is_empty());
    assert!(!env.data.path().join(LIBRARY_DIR).join("a.md").exists());
    assert_eq!(env.store.remove(&id).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn an_unknown_template_id_is_refused() {
    let env = Env::new();
    *env.template_id.lock().unwrap() = "nope".into();
    assert_eq!(
        env.store.placeholders(P).unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[test]
fn the_ui_component_library_hint_asks_for_library_primitives_and_install_command() {
    let marker = template::markers(REAL_TEMPLATE)
        .into_iter()
        .find(|m| m.label == "UI Component Library")
        .unwrap();
    assert!(!marker.optional);
    for part in [
        "the library",
        "its primitive layer (e.g. Base UI or Radix)",
        "the install command (e.g. the shadcn CLI",
    ] {
        assert!(
            marker.hint.contains(part),
            "{part} missing: {}",
            marker.hint
        );
    }
}
