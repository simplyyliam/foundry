### 📜 AGENTS.md - MASTER ARCHITECTURAL PROMPT CONTRACT (TEMPLATE)

> Usage: fill in every `[[ FILL IN: ... ]]` placeholder for a new project. Everything else (Laws, Exceptions, and most Conventions/Fail-Safes) is your reusable standard — leave it as-is. Named AGENTS.md, not CLAUDE.md — both Codex and current Claude Code versions read this filename natively; verify your installed Claude Code version still does before relying on it.
>
> Before the first hand-off: `docs/design/` and its manifest are populated automatically from your Design Library when `AGENTS.md` is generated (Foundry's P4) — nothing to copy by hand. Just fill in the `DESIGN FAMILY` line in the hand-off prompt so Section 6 knows which library files apply. If you're filling this template out by hand outside Foundry, put whatever guideline `.md` files apply into `docs/design/` yourself, plus a `manifest.md` listing them.

### 1. WORKSPACE TECH STACK & OPERATIONAL COMMANDS

```
[PROJECT STACK]
- Architecture: [[ FILL IN: e.g. pnpm Monorepo Workspace (Frontend UI + Server + Database Core) ]]
- Frontend Engine: [[ FILL IN: e.g. Vite / React / TypeScript ]]
- UI Component Library: [[ FILL IN: name all three, every time: the library (e.g. Tailwind CSS v4 +
  shadcn/ui), its primitive layer (e.g. Base UI or Radix), and the install command (e.g. the shadcn
  CLI: `pnpm dlx shadcn@latest add <component>`). Don't assume any of them is implied; a missing line
  here is exactly how a project shipped without it. ]]
- Backend Engine: [[ FILL IN: e.g. Express.js / TypeScript / Prisma ORM ]]
- Database Layer: [[ FILL IN: e.g. Postgres via Prisma, SQLite, etc. ]]
- Test Harness: Vitest (logic/unit) + Playwright (real browser interaction — see Law 9)

[CRITICAL COMMAND LIST]
- Boot Full Application Stack: `pnpm --filter "*" dev`
- Run Monorepo Build Check: `pnpm --filter "*" build`
- Generate Prisma Client: `pnpm --filter database prisma generate`
- Push Schema to Active DB: `pnpm --filter database prisma db push`
- Open Database Studio: `pnpm --filter database prisma studio`
- Target Frontend Feature Test: `pnpm --filter frontend vitest run src/features/<slice-name>/`
- Target Express Server Feature Test: `pnpm --filter server vitest run src/features/<slice-name>/`
(Adjust filter names/commands if a project's package names or backend framework differ.)
```

### 2. THE 9 LAWS OF THE PARALLEL MATRIX (VSM MANIFEST)

You are an execution developer agent strictly bound by the following structural rules:

1. **The Law of Parallel Co-Location:** Vertical slices cross both layers. When coding a capability, your actions are bounded strictly to matching feature directories in both frontend and server packages (packages/frontend/src/features/name/ and packages/server/src/features/name/). Horizontal layer files are strictly prohibited.
2. **The Parent-Child Hierarchy:** src/widgets/ inside the frontend package acts as the macro parent template layer. src/features/ acts as the micro functional children layer. Widgets assemble multiple child features together; features must never import from or control widgets.
3. **The Law of Vertical Slices:** Code generation columns must implement the child capability's frontend layout, calculation hooks, Express router endpoints, and corresponding database transactional queries simultaneously.
4. **The Sandbox Isolation Law:** Feature sandboxes are completely blind to each other. Cross-importing modules between different directories under src/features/ (on both server and frontend packages) is strictly prohibited.
5. **The Law of the State Contract:** Frontend and Server slices communicate exclusively via defined Express HTTP API paths. Persistent state shifts are managed solely by executing queries via the database client defined in packages/database/prisma/schema.prisma.
6. **The Law of Black Box Verification:** Vitest test files (*.spec.ts) must sit inline right next to the code inside the specific feature folder. Tests must strictly assert **Inputs vs. Outputs** using the **Arrange-Act-Assert** loop, clearing local mock states before each run.
7. **The Rule of Three:** Duplicate utility logic across slices to maintain complete feature sandbox independence. Do not move shared methods down to global package spaces unless identical blocks exist in 3+ features.
8. **The Horizon Shield:** Read the bottom "Future Horizon Primitives" block inside vsm_canvas.md / vsm_data.js. You are strictly forbidden from writing rigid interfaces that block these future design vectors.
9. **The Law of Traceable Requirements:** During Look Across, extract every explicitly-stated interaction, UX, and behavioral requirement from the source document — not just feature names, but specific stated behaviors (e.g. "hovering the icon previews its state," "Space plays/pauses," "the canvas never stops moving unless asked") — into an explicit checklist, each item assigned to exactly one slice. A slice is not "done" until every requirement assigned to it is checked off individually, not just until its own tests pass. Architectural cleanliness (Laws 1–8) and requirement completeness (this law) are different guarantees — passing tests proves the former, not the latter.

### 3. THE 3 ARCHITECTURAL CODE EXCEPTIONS

1. **The Shared Infrastructure Layer (packages/database/ & src/shared/):** Global Prisma database client schemas, universal layout typings, and base component-library primitives live here. Slices can import *downward* from this path.
2. **The Law of One-Way Dependency:** Shared infrastructure layers and UI packages are **strictly forbidden** from importing anything from widgets or features.
3. **The Router Entrypoint:** You are granted transient permission to modify packages/frontend/src/Router.tsx to mount view routes, and packages/server/src/index.ts to attach an active Express sub-router instance, then must retreat immediately back into your sandbox boundaries.

### 4. CODE DESIGN CONVENTIONS & DRIFT PREVENTION CONTRACT

To maintain production-grade compliance and eliminate stylistic drift, you must adhere to these structural rules:

* **Component Library, Non-Negotiable:** Every UI element must use whatever component library is named in Section 1 — never plain/hand-rolled HTML when a library component exists for it. Name the library explicitly in Section 1 every project; don't leave this implicit. How the UI should look is governed by Section 6.
* **Physical-Interaction Testing:** Any requirement describing a physical user interaction — hover, drag, a keyboard shortcut, scrubbing, continuous pointer movement — must be verified by a real Playwright browser test performing that actual physical interaction (real mouse down/move/up, a real keydown, a real mouseenter), not a Vitest unit test calling the underlying handler or store method directly. A passing unit test proves the logic is correct; it does not prove a user can actually do the thing.
* **Express Routing Encapsulation:** Every backend slice folder must export an isolated Express Router instance (name.router.ts). Do not pollute the main application file with individual endpoint logic.
* **Zero Ambiguity Typings:** Avoid using any or loose, implicit record types. All input form payloads, Express Request/Response params, and Prisma returns must be explicitly typed with strict TypeScript interfaces.
* **Error-As-Data Flow Architecture:** Do not let raw Express errors crash the node thread. Use result container schemas ({ success: true, data } | { success: false, error }) or pass errors down safely into custom Express error-handling middleware.
* **Deterministic Mocking Rules:** When writing tests, never create complex, erratic external API mocks. Use clean interface mock contracts inside the slice folder to guarantee deterministic test runs.
* **[[ FILL IN: Project-Specific Correctness Rule — optional ]]:** If this project has logic where a duplicate-but-inconsistent implementation would be dangerous (money math, scheduling, access control, etc.), name it here and require duplicates to pass identical test fixtures before merge. Delete this bullet if nothing in the project meets that bar.

### 5. RUNTIME ERROR BOUNDARY & FAIL-SAFE LOGISTICS

* **Fail Fast:** If you discover a missing type definition, broken runtime flag, or schema mismatch while reading a target slice, suspend code generation immediately. Do not guess parameters or write placeholder workarounds. Flag the structural database issue to the user.
* **Isolated Self-Correction Loop:** When a test fails (✕ failed), you are permitted to use local read/write tools *only* inside that active feature sandbox folder to apply corrections. If you cannot fix the bug within **2 iterative refactor steps**, stop execution, output the specific error log, and request developer guidance.
* **Strict Dependency Lock:** Do not attempt to upgrade, install, or alter workspace root package.json configurations or external npm modules unless explicitly ordered via a direct user prompt.
* **Install-Time Build Scripts:** pnpm may ignore install-time build scripts; if a package like Prisma or esbuild needs one, the foundation slice sets `pnpm.onlyBuiltDependencies` in the root package.json.
* **Cross-Slice Integration Check:** Per-slice tests are black-box and isolated under Law 6, so they cannot catch a contract mismatch between slices. Before the MVP is considered complete, write and run one integration test outside any single feature folder that exercises [[ FILL IN: your critical end-to-end path, e.g. "signup → checkout → confirmation" ]].

### 6. DESIGN REFERENCE

Design guideline files are the user's own input from their Design Library, copied into `docs/design/` for this project by `generate_agents`, along with a generated `docs/design/manifest.md` listing each file, its family tags, and a short description. This project has none if the folder is empty — proceed on tokens, the Bakery, and component-library defaults alone.

**Routing.** The project's design family is declared on the `DESIGN FAMILY` line of the hand-off prompt. Read `docs/design/manifest.md` and use whichever files' family tags match. If the family line is missing, or no library file matches it, ask — do not guess or invent design guidance.

**Reading discipline.** Read `docs/design/manifest.md` first — it's short. Open an individual guideline file only when a UI-touching slice needs what its description says it covers. Engine, backend, and data slices never read any of this.

**Taste references (Bakery).** `docs/bakery/manifest.md` lists the user's own reference images, each with what to take from it and what to skip. For UI-touching slices, read the manifest first and open an individual image only when its entry applies to what you are building. Apply only what an entry's "Take" line names; never copy a reference wholesale. Ignore entries marked `(unlabeled)` and say so in your report.

**Precedence, highest first:**
1. An explicit instruction in the current prompt
2. The reference screens in `docs/figma/` — matching tokens alone is not enough; compare the assembled result against the screens
3. The project's own token file (e.g. `index.css` `@theme`) — the single implementation of the tokens. Any token examples inside a guideline file are seeds to derive it from, never a second source of truth
4. The Bakery (`docs/bakery/manifest.md`) — the user's own taste references
5. The design guideline files in `docs/design/`, per the manifest's routing
6. Component-library defaults

UI direction precedence, highest first: the user's rulings (project rules), then the design family files and Bakery references, then the wording of requirements, then the brain dump.

**Principles, not templates.** These files study why designs work, often via a named reference product. Never reproduce a reference product's signature elements — its wordmark treatment, illustrations, layout composition, or brand details. Apply the principle (restraint, one accent, hierarchy through type and space) with this project's own identity.

**Gate.** In the first response of any UI-touching slice, state the design family and which manifest files you will apply before writing code — or that none matched, if that's the case. Before calling the slice done, if any matched file names a Do/Don't checklist, run it and report anything you could not satisfy. Design requirements in the hand-off count under Law 9: each is assigned to a slice and checked off individually.

### 7. TEAM

This project's agents are listed in `agents/roles.json`. Each has a role file at `agents/roles/<agent>.md` covering what it reads, what it may write, what it hands off, what done means, and what to do when blocked. If you were started with a message naming your role, read that role file first and stay inside its write boundaries. If `agents/roles.json` doesn't exist, no team has been confirmed yet: work from the task you were given. Never edit `agents/**` yourself; the team is changed through Foundry, not by agents.

---

### 🛠️ Day-One Entrypoint Setup (optional — replace if not using Express)

When you manually set up your boilerplate structure, use a clean, bare-bones entrypoint. Example for an Express backend:

```typescript
// packages/server/src/index.ts
import express from 'express';
import cors from 'cors';

const app = express();
const PORT = process.env.PORT || 3001;

app.use(cors());
app.use(express.json());

// Global Server Health Diagnostic
app.get('/health', (req, res) => {
  res.json({ status: 'active', timestamp: Date.now() });
});

// [EXCEPTION 3 ACTIVE ENTRYPOINT WINDOW]
// Future feature slice router registrations will be appended directly below this line:

app.listen(PORT, () => {
  console.log(`🚀 VSM Server executing smoothly on port ${PORT}`);
});
```