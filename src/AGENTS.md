# AGENTS.md — Foundry frontend (src/)

You work on Foundry's React frontend in `src/`. A separate agent owns the Rust backend in `src-tauri/`. Read `src/DECISIONS.md` before building on earlier UI work.

## Boundaries
- Edit only `src/`. Never edit `src-tauri/`.
- `src/lib/bindings.ts` is generated (a debug run of the app re-exports it). Never edit it. If a screen needs data or a command the bindings don't provide, stop and tell me what's missing. Don't work around it.
- Never run anything that uses my Claude subscription (`generateRoadmap`, `composeTeam`, `agentStart`) unless the prompt asks for it.
- Add no npm dependency without asking. Adding a shadcn component is fine unless it needs a new npm package.
- Don't commit unless asked.

## Structure
- A widget (`src/widgets/<name>/<Name>.tsx`, or `src/widgets/<name>.tsx`) is a thin shell assembling `src/features/<name>/components/`. It imports only its own feature, `@/shared` and `@/components/ui`. Widgets never import each other.
- Features never import widgets, or each other. One exception: `@/features/projects` is the shared project model (store, `Project`, `PROJECT_SECTIONS`, `ProjectRoute`) and any feature may use it.
- Pages are file-based (`src/pages/app/**/page.tsx`; `[param]` folders become `:param`). New page folders aren't picked up by a running dev server: restart it. Pages set breadcrumbs with `usePageHeader`.
- Layouts (`src/pages/app/layout/`) place widgets side by side. A frame that composes widgets lives there, like the contextual panel (`layout/contextual-panel/`).
- New code calls `commands.*` or listens to events only in `src/features/<name>/hooks/`. Components get data from props or their feature's hooks. Older exceptions: `features/projects/model/store.ts`, the add-project folder picker, and `appInfo` in `App.tsx`.
- Two features that need the same data each fetch it themselves and refetch on the backend's event (e.g. `plan:changed`), rather than importing each other.
- Per-project screens take the project id from the route (`/projects/:projectId/...`), never from a shared "active project".
- The route decides which contextual-panel views are possible, via `ROUTE_VIEWS` / `PANEL_VIEWS` in `layout/contextual-panel/views.tsx`. On project routes the panel title is a dropdown between that project's views (`PROJECT_VIEW_IDS`: Roadmap, References); the choice lasts the session and isn't saved. Never a cross-context view: no project view on a route without `:projectId`, no Agent Chats on a project page, and no switcher on non-project routes.

## Backend calls
- Every command resolves to `{ status: "ok", data } | { status: "error", error: AppError }`. Also catch throws: a real IPC failure throws. Wrap calls in an `Outcome`-style helper, like `call()` in `features/roadmap/hooks/call.ts`.
- Show the backend's error message as-is. Don't invent wording and don't fail silently.
- `updateSettings` replaces the whole object: send back what `getSettings` returned, changing only your field.

## UI
- New controls use shadcn/ui and Tailwind v4 theme tokens: no plain HTML controls, no hardcoded colors. Some older code predates this (see DECISIONS.md).
- Match the mockups. Every clickable gets the subtle press-scale feedback: reuse `usePressScale` from `@/shared`.
- Motion: transform/opacity/blur only, nothing that reflows. Respect the OS reduced-motion setting.
- Add obvious polish without being asked (keyboard paths, good empty states, inline "Create …" from a search), and list each unrequested touch in your report so I can veto it.

## Reporting
- You can't click inside the Tauri window. Say what you couldn't check by eye. A passing type-check doesn't mean it works.
- List each requirement in the prompt as met / partly / not met, and say how you checked it.
- Append one line to `src/DECISIONS.md` for any decision that isn't obvious from the code.
