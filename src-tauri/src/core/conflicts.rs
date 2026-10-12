//! Conflicts between what the brain dump says about the UI and the project's references (its
//! design family files and Bakery notes). Roadmap generation finds them and saves them in
//! `.foundry/roadmap.json`; `load_roadmap` carries them into the plan, where they're ruled on.

use serde::{Deserialize, Serialize};
use specta::Type;

/// At most this many conflicts are kept from one generation.
pub const MAX_CONFLICTS: usize = 12;

/// The fixed order for UI direction. One line in AGENTS.md's design section and in the
/// hand-off's project rules.
pub const PRECEDENCE_LINE: &str = "UI direction precedence, highest first: the user's rulings \
     (project rules), then the design family files and Bakery references, then the wording of \
     requirements, then the brain dump.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ConflictSourceKind {
    /// A design guideline file in `docs/design/`.
    DesignFamily,
    /// A Bakery entry's Take or Skip note.
    Bakery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSource {
    pub kind: ConflictSourceKind,
    /// `design_family`: the file name in `docs/design/` (e.g. `dashboard.md`).
    /// `bakery`: the entry's image file name in `docs/bakery/` (e.g. `03-red-door.png`).
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RulingKind {
    ReferenceWins,
    BrainDumpWins,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictRuling {
    pub kind: RulingKind,
    /// Required for `custom` (the ruling itself); otherwise an optional note.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapConflict {
    pub id: String,
    /// What it's about, e.g. "minimap".
    pub topic: String,
    /// Found word for word (whitespace-normalized) in `docs/brain-dump.md`.
    pub brain_dump_excerpt: String,
    pub other_source: ConflictSource,
    /// Found word for word (whitespace-normalized) in that source.
    pub other_excerpt: String,
    /// Always `reference_wins`.
    pub suggested: RulingKind,
    /// Null until `resolve_roadmap_conflict`.
    #[serde(default)]
    pub ruling: Option<ConflictRuling>,
    /// The project-wide constraint made from the ruling (`addAsRule`), if any.
    #[serde(default)]
    pub rule_requirement_id: Option<String>,
}
