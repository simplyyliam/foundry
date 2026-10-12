Hand-off Prompt

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
APPLICATION PROFILE: A reading app that syncs highlights.
WORKSPACE ARCHITECTURE: pnpm monorepo
BACKEND TIERS: Express / TypeScript
FRONTEND TIER: Vite / React / TypeScript
DESIGN FAMILY: saas dashboard
PERSISTENCE LAYER: Postgres via Prisma

CORE MVP USER CONSTRAINTS & FEATURE PRIMITIVES:
- Sign in
  - r-1: Signing in with a magic link takes one click.
- Feed
  - r-2: The feed shows the newest highlight first.
  - r-3: Pressing J moves to the next highlight; K moves back.
- Share
  - r-4: A shared link opens without signing in.

PROJECT RULES (apply to everything built, in every slice):
- UI direction precedence, highest first: the user's rulings (project rules), then the design family files and Bakery references, then the wording of requirements, then the brain dump.

FUTURE HORIZON ROADMAP PRIMITIVES:
- Offline mode (anchored to: Feed)

REFERENCES:
- (none)
==================================================================================

---

### 6. INSTRUCTIONS FOR THE EXECUTION AGENT (HAND-OFF PROTOCOL)
You are now bound by the structural layout and code rules of this monorepo project contract. Proceed immediately using the following sequence:
1. Acknowledge: Confirm you recognize the architectural constraints, monorepo package splits, and folder layout definitions.
2. Execute Phase 1 (The Look Across): Read the application data inside the [PROJECT MEMORY CAPTURE FIELD] above. Do not generate functional routes, components, or Prisma queries inside `packages/` yet.
3. Generate the Data: Populate `vsm_data.js` in the project root, following the schema documented in its own header comment — one entry per vertical slice (id, name, order/tier, dependencies, delivered capabilities, status), plus a `future_horizon` entry for every deferred feature listed above, each anchored to the slice whose architecture must not block it later.
4. Do Not Touch the Viewer: `vsm_viewer.html` already exists in the project root and is fixed — it reads `vsm_data.js` and renders the matrix. Do not create, rewrite, or restyle it. Once `vsm_data.js` is populated, open `vsm_viewer.html` directly in a browser to view the result.

Output confirmation that `vsm_data.js` was populated, how many slices and future-horizon items were added, and any dependency you were unsure how to assign — flag it rather than guessing.
