Hand-off Prompt Template

> Usage: fill in every `[[ FILL IN: ... ]]` placeholder inside the [PROJECT MEMORY CAPTURE FIELD] below for a new project — that's your brain dump. Everything else is your reusable standard and shouldn't need edits unless a project genuinely uses a different folder layout.
>
> Before sending this prompt: copy `vsm_viewer.html` (fixed, never regenerated) and rename `vsm_data.template.js` to `vsm_data.js` (blank, the agent fills this in) into the project root, alongside `AGENTS.md`/`CLAUDE.md`.

### 1. EXECUTIVE ARCHITECTURAL SUMMARY
This project is strictly governed by the Vertical Slice Matrix (VSM) system architecture. The development lifecycle must move through three distinct phases for every business capability: Look Across (Plan), Isolate (Context), and Slice (Execute). The primary purpose is to maximize model efficiency, drive token usage down, and eliminate architectural drift via strict parent/child (Widgets/Features) co-location across the pnpm monorepo packages.

---

### 3. THE 3 ARCHITECTURAL MONOREPO EXCEPTIONS
1. The Shared Infrastructure Layers: Global data contracts live inside `packages/frontend/src/shared/types.ts` and database structural models live in `packages/database/prisma/schema.prisma`. Feature slices can import downward from these directories.
2. The Law of One-Way Dependency: Global shared folders, component UI roots, and base database schemas are strictly forbidden from importing anything from individual widgets or features.
3. The Wiring Entrypoints: The agent is granted transient permission to exit a sandbox solely to register finished paths inside `packages/frontend/src/Router.tsx` (frontend views) and `packages/server/src/index.ts` (Express sub-routers), then must retreat immediately back into its feature sandbox.

---

### 5. CURRENT PROJECT DATA SYNC & SYSTEM HAND-OFF
========================= [PROJECT MEMORY CAPTURE FIELD] =========================
APPLICATION PROFILE: [[ FILL IN: what is this app, in one or two sentences ]]
WORKSPACE ARCHITECTURE: [[ FILL IN: e.g. pnpm Monorepo Layout (packages/frontend, packages/server, packages/database) ]]
BACKEND TIERS: [[ FILL IN ]]
FRONTEND TIER: [[ FILL IN ]]
DESIGN FAMILY: [[ FILL IN: SaaS/web dashboard · playful visual editor · native desktop app · micro-utility · marketing site — pick per AGENTS.md §6, more than one if the project has both an app and a landing page ]]
PERSISTENCE LAYER: [[ FILL IN ]]

CORE MVP USER CONSTRAINTS & FEATURE PRIMITIVES:
- [[ FILL IN: describe every feature/behavior in scope right now, in as much detail as you want — this is the raw brain dump the Look Across phase reads ]]

FUTURE HORIZON ROADMAP PRIMITIVES:
- [[ FILL IN: features you deliberately aren't building yet — listed so the agent designs the architecture to allow for them later without building them now ]]

REFERENCES:
- [[ FILL IN: the Bakery entries that apply, each with its Take and Skip notes copied word for word ]]
==================================================================================

---

### 6. INSTRUCTIONS FOR THE EXECUTION AGENT (HAND-OFF PROTOCOL)
You are now bound by the structural layout and code rules of this monorepo project contract. Proceed immediately using the following sequence:
1. Acknowledge: Confirm you recognize the architectural constraints, monorepo package splits, and folder layout definitions.
2. Execute Phase 1 (The Look Across): Read the application data inside the [PROJECT MEMORY CAPTURE FIELD] above. Do not generate functional routes, components, or Prisma queries inside `packages/` yet.
3. Generate the Data: Populate `vsm_data.js` in the project root, following the schema documented in its own header comment — one entry per vertical slice (id, name, order/tier, dependencies, delivered capabilities, status), plus a `future_horizon` entry for every deferred feature listed above, each anchored to the slice whose architecture must not block it later.
4. Do Not Touch the Viewer: `vsm_viewer.html` already exists in the project root and is fixed — it reads `vsm_data.js` and renders the matrix. Do not create, rewrite, or restyle it. Once `vsm_data.js` is populated, open `vsm_viewer.html` directly in a browser to view the result.

Output confirmation that `vsm_data.js` was populated, how many slices and future-horizon items were added, and any dependency you were unsure how to assign — flag it rather than guessing.
