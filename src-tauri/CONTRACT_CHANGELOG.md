# Contract changelog

Every change to a Tauri command or event (the frontend contract) is logged here: name, before → after, why.
Additive changes are fine. Changing or removing an existing entry needs the human's approval.
Types are generated into `src/lib/bindings.ts` (tauri-specta); see that file for exact shapes.

## P0 — backend skeleton

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `AppError` | — → `{ code: ErrorCode, message: string }` | The error type every command returns. |
| type | `ErrorCode` | — → `"invalid_path" \| "path_escape" \| "io" \| "internal"` | Stable, machine-readable error codes. New codes are additive. |
| type | `EventMeta` | — → `{ id: string, ts: number }` | Carried by every event payload (flattened). `ts` is epoch milliseconds. |
| command | `app_info` | — → `appInfo(): Result<AppInfo, AppError>` where `AppInfo = { version: string, os: string }` | Proves the command round trip. `os` is Rust's `std::env::consts::OS` (`"windows"`, `"macos"`, `"linux"`). |
| event | `app:ready` | — → `AppReady = { id, ts, version: string }` | Proves the event round trip. Emitted once at the end of backend setup. Best-effort: it may fire before the frontend listens, so the frontend calls `appInfo()` on mount instead of waiting for it (decided by the human; no retry/replay mechanism). |

## Folder picker (read-only)

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `list_folders` | — → `listFolders(path: string \| null): Result<FolderListing, AppError>` where `FolderListing = { path, parent: string \| null, isGitRepo, folders: FolderEntry[] }`, `FolderEntry = { name, path, isGitRepo }` | Powers the custom (styleable) folder picker in the add-project modal; the native OS dialog can't be styled. `null` lists the home folder. Relative paths and `..` → `invalid_path`. Hidden folders and files are omitted. Never writes. |
| command | `folder_locations` | — → `folderLocations(): Result<FolderLocations, AppError>` where `FolderLocations = { home: string, roots: string[] }` | Quick locations for the picker: home folder and drive roots (`C:\`) or `/`. |

## P1 — projects

All additive. Every command returns `Result<T, AppError>`.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"invalid_input" \| "not_found" \| "folder_missing" \| "folder_exists" \| "already_linked" \| "write_not_allowed"` | `folder_exists` / `already_linked` are the spec's FolderExists / AlreadyLinked. `folder_missing`: a registered project's folder is gone, so the UI can offer relink. `write_not_allowed`: a repo write outside the AGENTS.md allowlist. |
| type | `Project` | — → `{ id, name, tagline, repoPath, lastOpened: number \| null, folderExists: boolean }` | Registry entry as the UI sees it. `lastOpened` is epoch ms. `folderExists` is computed on read (not stored). |
| type | `Settings` | — → `{ maxConcurrentAgents: number, defaultProjectsDir: string \| null, claudePath: string \| null }` | App settings. Defaults: 3 / null / null. Paths must be absolute. |
| command | `list_projects` | — → `listProjects(): Project[]` | Registered projects, in the order added. |
| command | `create_repository` | — → `createRepository(input: { name, parentDir: string \| null }): { project, gitInitialized, initialCommit }` | Makes `<parentDir>/<slug(name)>` (`parentDir` defaults to `defaultProjectsDir`; neither → `invalid_path`), `git init` plus one empty commit, then `.foundry/project.json`, `docs/brain-dump.md` and `vsm_viewer.html`. Existing folder → `folder_exists`, nothing touched. Registers but doesn't open. |
| command | `link_repository` | — → `linkRepository(input: { folder }): { project, isGitRepo, createdFiles: string[] }` | Registers an existing folder and adds only missing Foundry files; never runs git. A folder with a `.foundry/project.json` keeps its id, name and tagline. `createdFiles` (beyond the spec) tells the UI what was added. |
| command | `update_project` | — → `updateProject(id, patch: { name: string \| null, tagline: string \| null }): Project` | Name/tagline in the registry and `.foundry/project.json` (other fields kept). Never renames or moves the folder. |
| command | `open_project` | — → `openProject(id): Project` | Makes it active and sets `lastOpened`. `folder_missing` if the folder is gone. |
| command | `active_project` | — → `activeProject(): Project \| null` | In memory only: null after restart until `open_project` (decided by the human). |
| command | `relink_project` | — → `relinkProject(id, folder): Project` | Points a project at its moved folder; adds missing Foundry files. Rejects a folder registered to another project (`already_linked`) or holding a different project's `project.json` (`invalid_input`). |
| command | `remove_project` | — → `removeProject(id): null` | Unregisters only; never deletes repo files. Clears the active project if it was this one. |
| command | `get_document` | — → `getDocument(id): string` | `docs/brain-dump.md`; empty string if the file is missing. |
| command | `save_document` | — → `saveDocument(id, markdown): null` | Atomic replace of `docs/brain-dump.md`. |
| command | `get_settings` | — → `getSettings(): Settings` | |
| command | `update_settings` | — → `updateSettings(settings: Settings): Settings` | Whole-object replace; returns what was saved (blank paths → null). `maxConcurrentAgents` must be ≥ 1. |

## Project archiving

`remove_project` is a breaking behaviour change, approved by the human. Everything else is additive.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"not_archived"` | Returned when an action needs the project archived first. |
| type | `Project` | adds `archived: boolean` | Foundry-side flag stored in `projects.json` (missing in older files → `false`). Never touches the folder. |
| command | `list_projects` | unchanged signature; each `Project` now carries `archived` | Returns archived and active projects alike (no server-side filter); the frontend splits them. |
| command | `archive_project` | — → `archiveProject(id): Project` | Sets `archived = true`. Registry only; idempotent. `not_found` for an unknown id. If it's the active project it's also closed (`activeProject()` → null, same as no project open); no event is emitted. Refuses with `session_running` while the project has a live agent session (added in P7, see below). |
| command | `unarchive_project` | — → `unarchiveProject(id): Project` | Sets `archived = false`. This is the restore action (nothing left disk). Idempotent. `not_found` for an unknown id. |
| command | `remove_project` | `removeProject(id): null` (any project) → `removeProject(id): null`, **archived projects only**; otherwise `not_archived` and nothing changes | The backend enforces "you can only delete what you've already archived". Still unregisters only and never deletes repo files. Chosen instead of adding a new `delete_project` (the human decided). |

## Delete to Recycle Bin

Additive: one new command. It replaces R-1-5 ("removing never deletes repo files") **for this command only**. `remove_project` is unchanged: it still only unregisters, archived projects only (test: `remove_project_still_only_unregisters_alongside_delete_repository`).

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `delete_repository` | — → `deleteRepository(id): Result<null, AppError>` | Moves an archived project's folder to the Recycle Bin, then unregisters it and clears the active project if it was this one. Folder already gone → just unregisters. Errors: `not_found` (unknown id), `not_archived` (not archived; nothing changes), `io` if the Recycle Bin move fails, e.g. a file is open elsewhere: `could not move <path> to the Recycle Bin: a file in it is open in another program; close it and try again (0x80270028)`. Other failures carry the OS's message and HRESULT. On `io` the folder and the registration are untouched. If Windows can't recycle the folder (e.g. too big for the bin), it shows its own "delete permanently?" prompt instead of deleting silently; declining → `io` "…was not moved to the Recycle Bin (cancelled)". Windows only for now: other OSes always get `io` "…isn't supported on this OS yet". Uses the existing `windows` 0.62 dependency with `Win32_System_Com` and `Win32_UI_Shell` turned on (no new crate). |

## P5 — Bakery

All additive. Every command takes `projectId` (decided by the human) and returns `Result<T, AppError>`. Every command that changes the Bakery returns `unlabeledCount`.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"bakery_full" \| "unsupported_image" \| "file_too_large"` | `bakery_full` is the spec's BakeryFull. `unsupported_image`: not PNG/JPEG/WebP by content, or undecodable. `file_too_large`: input over 25 MB. |
| type | `Coord` | — → `number` | Canvas coordinate/size. Always finite (rejected otherwise), so it exports as `number`; a bare `f64` would export as `number \| null`. |
| type | `BakeryItem` | — → `{ id, file, label, take, skip, tags: string[], appliesTo: string[], x, y, w, h: Coord, z: number }` | One image. `file` is a bare name inside `docs/bakery/` (`NN.ext` until labelled, then `NN-<slug>.ext`, `-2`… on a collision). |
| type | `BakeryState` | — → `{ items: BakeryItem[], unlabeledCount: number, maxImages: number, dir: string }` | `items` ordered by `NN`. `unlabeledCount` = images with no label or no Take note. `maxImages` from `bakery.json` (default 40, no command changes it yet). `dir` is the absolute `docs/bakery/` path so the UI can show `<dir>/<file>`. |
| type | `BakeryAddResult` | — → `{ added: string[], bakery: BakeryState }` | `added` = new item ids, in input order. |
| type | `DropPoint` | — → `{ x: Coord, y: Coord }` | |
| type | `BakeryPatch` | — → `{ label, take, skip: string \| null, tags, appliesTo: string[] \| null }` | `null` leaves a field unchanged. Label ≤ 100 chars. Lists are trimmed and de-duplicated. |
| command | `bakery_list` | — → `bakeryList(projectId): BakeryState` | Never writes. |
| command | `bakery_add_from_paths` | — → `bakeryAddFromPaths(projectId, paths: string[], dropPoint: DropPoint \| null): BakeryAddResult` | Dropped files (absolute paths). All or nothing: one bad file adds none. First card at `dropPoint` (or a cascade from the origin), the rest offset by 24. Cards are 240 on the long edge, aspect kept. |
| command | `bakery_add_from_bytes` | — → `bakeryAddFromBytes(projectId, name, bytes: number[]): BakeryAddResult` | Paste. `name` must be a plain file name (`path_escape` otherwise); the format comes from the bytes. |
| command | `bakery_update` | — → `bakeryUpdate(projectId, id, patch): BakeryState` | A label change renames the file, then saves the layout and rewrites the manifest. If the layout save fails, the rename is undone. |
| command | `bakery_move` | — → `bakeryMove(projectId, id, x: Coord, y: Coord, w, h: number \| null, z: number \| null): BakeryState` | Layout only: `manifest.md` is never rewritten. `w`/`h` must be > 0. |
| command | `bakery_remove` | — → `bakeryRemove(projectId, id): BakeryState` | Deletes only the item's file inside `docs/bakery/`. A `bakery.json` entry pointing elsewhere → `path_escape`, and nothing changes. |

Errors common to every Bakery command: `not_found` (unknown project or item id), `folder_missing` (project folder gone), `invalid_input`, `io`.

Internal (not contract): the projects feature now publishes id → repo folder to `core::repos::RepoIndex`, which the Bakery reads (Law 1). `slug` moved to `core::slug`. `core::repo_write` gained allowlisted `remove` and `rename_new`. Dependency added (approved by the human): `image` 0.25 with only `png`, `jpeg`, `webp`.

### Displaying Bakery images (asset protocol)

No command or type changed. The UI shows an image with `convertFileSrc(<dir>/<file>)` from `@tauri-apps/api/core`, using `BakeryState.dir` and `BakeryItem.file`.

- `tauri` gains its `protocol-asset` feature, and `tauri.conf.json` sets `app.security.assetProtocol = { enable: true, scope: [] }`. The static scope stays empty.
- At runtime, each project's `docs/bakery/` folder is added to the scope (not recursively) the first time any Bakery command touches it after the folder exists. Only files directly inside it load. The rest of the repo, `.foundry/`, subfolders and links pointing elsewhere are refused (the scope resolves links before matching).
- `remove_project` and `delete_repository` both revoke the grant: once the folder leaves the registry, its `docs/bakery/` is forbidden, recursively, whether or not the files remain on disk. Tauri 2.11 has no way to remove a granted folder; a forbid always overrides a grant, so this is the revocation. A forbid can't be lifted at runtime either. If the same folder is linked again in that session (after a remove, or after a restore from the Recycle Bin), its Bakery images won't display until the app restarts.
- `csp` is `null`, so nothing else is needed. If a CSP is ever set, it must allow `img-src asset: http://asset.localhost`.

## P7 — agent sessions (engine; launch pending P3/P6/P8)

All additive. `agent_start` is a stub until the roster and launch message (P6), the subscription preflight (P3) and the hook server (P8) exist. The other commands and both events are real, working against sessions the engine runs.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"session_running" \| "too_many_sessions" \| "not_implemented"` | `session_running` (approved in P1): an agent session is live, either for the project being archived or for the agent being started. `too_many_sessions` is the spec's TooManySessions. `not_implemented`: the `agent_start` stub, which goes away once P3/P6/P8 land; removing it then is a contract change for approval. |
| type | `SessionStatus` | — → `"idle" \| "starting" \| "running" \| "exited"` | |
| type | `SessionInfo` | — → `{ agentId, projectId, status: SessionStatus, pid: number \| null, since: number }` | A live session. `since` = epoch ms of the last status change. |
| type | `AgentStartOptions` | — → `{ promptFile: string \| null, sliceId: string \| null }` | |
| type | `AgentStopOptions` | — → `{ force: boolean }` | |
| command | `agent_list` | — → `agentList(): SessionInfo[]` | Live sessions, ordered by agent id. Idle roster agents join once P6 exists. |
| command | `agent_start` | — → `agentStart(agentId, options): SessionInfo` | **Stub:** always `not_implemented`, starts nothing. |
| command | `agent_write` | — → `agentWrite(agentId, data: string): null` | Keystrokes or pasted text. `not_found` if the agent has no running session. |
| command | `agent_resize` | — → `agentResize(agentId, cols, rows): null` | Both ≥ 1 (`invalid_input`). |
| command | `agent_stop` | — → `agentStop(agentId, options): null` | Sends Ctrl+C, waits 5 s, then kills the whole process tree (`force`: kill at once). Resolves once the process is gone. Stopping an agent that already exited is fine; one that never ran → `not_found`. |
| command | `agent_snapshot` | — → `agentSnapshot(agentId): string` | Base64 of the newest ≤ 256 KB of output, for a remounting terminal. Empty if the agent hasn't run. Kept after exit, until the agent starts again. Memory only, never on disk. |
| event | `agent:output` | — → `AgentOutput = { id, ts, agentId, chunk: string }` | `chunk` = base64 of raw terminal bytes, coalesced into ~16 ms batches (≤ 64 KB). A batch may split a UTF-8 sequence or an escape code, so feed it to the terminal as bytes. |
| event | `agent:status` | — → `AgentStatus = { id, ts, agentId, status, exitCode: number \| null, pid: number \| null, since }` | `starting` → `running` (with `pid`) → `exited` (with `exitCode` when known; 1 when killed). A failed spawn goes `starting` → `exited`. |
| command | `archive_project` | now `session_running` while a session is live for the project | Closes the P1 gap. Nothing changes when refused. |

Internal (not contract): `core::sessions::SessionRegistry` holds live session state (`reserve`, `set_running`, `release`, `is_project_active`, `list`), shared by the agent-sessions engine and projects (Law 1). `ProjectsStore::load` takes it as a required argument. Dependencies added (approved): `portable-pty` 0.9, `base64` 0.22, and `windows` features `Win32_Security` (required by `CreateJobObjectW`), `Win32_System_JobObjects` and `Win32_System_Threading`.

## P3 — roadmap generation (plan loading pending P2)

All additive. Loading the result into the plan is done by the plan's `load_roadmap` (see "P3's plan half" below). `generate_roadmap`'s signature is unchanged.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"claude_not_found" \| "not_signed_in" \| "wrong_auth_mode" \| "cancelled" \| "timed_out" \| "generation_failed" \| "generation_running"` | The first three are the spec's ClaudeNotFound / NotSignedIn / WrongAuthMode (wrong mode = signed in with an API key, the console or a cloud provider). `generation_failed`: output invalid twice (the message lists every error), or Claude Code reported an error or crashed. `generation_running`: one generation per project at a time. |
| type | `ClaudeStatus` | — → `{ path, version, loggedIn, authMethod, apiProvider, subscriptionType, problem: AppError \| null }` | Never includes the account's email or organisation. |
| type | `Roadmap` | — → `{ goal, targetUsers[{ name, description, painPoints }], featureMap[{ family, description, features, relatesTo }], slices[{ name, delivers, dependsOn }], requirements: Requirement[], deferred[{ label, anchoredTo }], openQuestions }` | As the prompt demands. `requirements` holds only those whose `sliceName` names a slice. |
| type | `Requirement` | — → `{ text, kind, sourceExcerpt, sliceName: string \| null, verifiedBy, unsourced: boolean }` | `unsourced`: the excerpt isn't a verbatim (whitespace-normalized) substring of the brain dump, or it's empty. Kept, never dropped. |
| type | `RoadmapResult` | — → `{ roadmap, unassigned: Requirement[], uncoveredParagraphs: string[], attempts: number, generatedAt: number }` | Saved as `.foundry/roadmap.json`. `uncoveredParagraphs`: brain-dump paragraphs (blank-line separated; headings and `---` rules excluded) that no sourced requirement's excerpt falls inside. |
| type | `Stage` | — → `"preflight" \| "running" \| "validating" \| "retrying" \| "done" \| "failed"` | |
| event | `roadmap:progress` | — → `RoadmapProgress = { id, ts, projectId, stage, elapsedMs }` | `projectId` added beyond the spec, so the UI can tell projects apart. |
| command | `generate_roadmap` | — → `generateRoadmap(projectId): RoadmapResult` | Preflight → `claude -p` → validate. One retry with the errors appended; a second failure saves the raw output to `.foundry/last-generation-error.txt` and returns `generation_failed`. Nothing else is written unless validation passes. Times out after 5 minutes (`timed_out`). An empty brain dump → `invalid_input`. |
| command | `cancel_generation` | — → `cancelGeneration(projectId): boolean` | Kills the run's process tree; the generation then fails with `cancelled`. Returns whether one was running. |
| command | `get_roadmap` | — → `getRoadmap(projectId): RoadmapResult \| null` | |
| command | `claude_status` | — → `claudeStatus(): ClaudeStatus` | Never fails: problems are reported in `problem`. |

Validation errors that trigger the retry: invalid JSON (a code fence around it is fine), missing or mistyped fields (each one reported with its path), duplicate slice names, a `dependsOn` that names no slice, a slice depending on itself, and dependency cycles (reported as `A → C → B → A`).

Internal (not contract): `core::subscription::subscription_env()` and `core::claude` (`find`, `auth_status`, `check_auth`, `preflight`, `status`, and `run` with deadline, cancel and tree kill) are shared for P7. `ProcessTree` moved from `features/agent_sessions` to `core::process_tree` (unchanged) so both features can kill a process tree. New test binary `fake-claude`. P7 engine fix: a write right after `agent_stop` returned could succeed into a dead pipe; the writer now closes before the exit is announced.

### P3 follow-up — two Settings fields (decided by the human)

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `Settings` | `{ maxConcurrentAgents, defaultProjectsDir, claudePath }` → adds `defaultModel: string \| null` and `generationTimeoutMs: number` | `defaultModel` is passed to `claude -p` as `--model <name>` when set (blank → null). `generationTimeoutMs` (≥ 1, default 300000) replaces the fixed 5-minute generation limit. Both are read at the start of every generation. `updateSettings` replaces the whole object, so the UI must now send both fields. A `settings.json` without them, or with `generationTimeoutMs: 0`, loads with null and 300000. |

### P7 note — `agent_stop` timing after the write-race fix (accepted by the human)

`agent_write` after `agent_stop` resolves, or after `agent:status` reports `exited`, is always refused with `not_found` (test: `writes_after_an_exit_is_observed_are_always_refused`). To guarantee that, a session's input writer now closes before the exit is announced.

Known theoretical risk, accepted as a trade-off: if an `agent_write` is blocked on a full terminal input pipe at the moment the process exits, the exit can't be announced until that write finishes. `agent_stop` then waits out its 3-second kill wait and returns `internal` ("still running after being killed"), even though the process is gone. ConPTY keeps draining input until the terminal closes, so this isn't expected in practice. Not reproduced, and deliberately not chased further.

## P7 follow-up — launching agents (engine level; `agent_start` still pending P6/P8)

`agent_start` is unchanged: still the `not_implemented` stub. It needs P6 (which project an `agentId` belongs to, and its `launch_message`) and P8 (the hook port and token). The launch path it will call, `SessionManager::launch`, is built and tested.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"project_archived"` | Starting an agent in an archived project. Mirrors `archive_project`'s `session_running`: same `{ code, message }` shape; message "unarchive this project before starting an agent in it". |
| command | `archive_project` | same signature and errors; the `session_running` check is now atomic with archiving | It checks for live sessions and saves the archived flag while holding the session registry's lock. A launch takes the same lock to reserve its slot. Archive and launch on the same project can't both succeed (test: `archive_and_launch_racing_never_both_succeed`). |

What `launch` will give `agent_start`, in order: preflight (`claude_not_found` / `not_signed_in` / `wrong_auth_mode`, via `core::claude::preflight(claudePath)`; nothing spawned) → repo (`not_found` / `folder_missing`) → slot reserved only if the project isn't archived (`project_archived`) → `claude [--model <defaultModel>] "<launch message>"` in the repo. The environment is `core::subscription_env()` plus exactly `FOUNDRY_AGENT_ID`, `FOUNDRY_PROJECT_ID`, `FOUNDRY_HOOK_PORT`, `FOUNDRY_HOOK_TOKEN` and `TERM=xterm-256color` (inherited ones of those names are replaced). Agent launches never run `claude` through a `.cmd`/`.bat` shim (npm's `claude.cmd`): they return `claude_needs_native`, whose message gives the fix (run `claude install`, then set Claude path to the installed claude.exe). Superseded: an earlier character blocklist for shims was not provably complete for cmd.exe parsing, and missed `--model`.

Internal: `core::repos::RepoIndex` also carries which projects are archived, published by projects with the rest of the index. `core::sessions` gains `reserve_if` and `unless_project_active` (one lock for both checks). P3 fix: the roadmap prompt template is sent with LF line endings whatever the checkout's (git autocrlf had made it CRLF on this branch).

### Shim hardening (decided by the human)

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"claude_needs_native"` | An agent launch found the npm `.cmd` shim. Windows runs shims through cmd.exe, which re-parses the launch message and `--model`; no character filter can be shown to be complete, so shims aren't used for agent sessions. The message names the fix: run `claude install`, then set Claude path to `%USERPROFILE%\.local\bin\claude.exe`. P3's `claude -p` is unaffected: std's `Command` escapes arguments for `.cmd` files (test: `model_through_a_cmd_shim_arrives_verbatim`). |
| type | `Settings` | `defaultModel` now accepts only ASCII letters, digits and `. - _ : / @ [ ]` (≤ 100) | Model names never need more, so neither launch path can receive a cmd.exe special character. `update_settings` → `invalid_input` otherwise; a hand-edited invalid value in `settings.json` loads as null. |

## P2 — the plan (`.foundry/plan.json`) and its `vsm_data.js` mirror

All additive. Every command takes `projectId` (the Bakery convention, decided by the human). Every mutation takes `expectedRevision: number | null` and emits `plan:changed`.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"stale_revision" \| "dependency_cycle" \| "dependencies_not_done" \| "requirement_unassigned" \| "slice_in_use"` | `stale_revision` = the spec's StaleRevision. `dependency_cycle` names the cycle (`A → C → B → A`). `slice_in_use`: requirements assigned (the spec's rule), plus dependents or anchored deferred items, so links are never dropped silently. |
| type | `PlanSlice`, `PlanRequirement`, `PlanDeferred` | — → the spec's Slice / Requirement / Deferred | Exported with a `Plan` prefix: P3's roadmap already exports draft types named `Slice`, `Requirement` and `Deferred`. `openQuestions` are `{ id, text }`, so they can be resolved by id. `startCommit` = HEAD when a slice moves to `in_progress` (null outside git). |
| type | `SliceStatus`, `RequirementKind`, `RequirementState` | — → `queued \| in_progress \| done`, `behavior \| interaction \| constraint`, `proposed \| confirmed` | |
| type | `Plan` | — → `{ revision, slices: SliceView[], requirements, deferred }` | `SliceView` = a `PlanSlice` plus derived `layer` (0 = no dependencies; longest path from a root), `usedBy` (number of dependents) and `ready` (queued, all dependencies done). Derived values are never stored. |
| type | `PlanAdded` | — → `{ id, plan }` | Returned by every `add_*` command. |
| type | `PlanSummary` | — → `{ unassigned, unconfirmed, ready, blocked, acyclic, confirmed }` | `confirmed` only with ≥ 1 slice, an acyclic graph, and every requirement assigned and confirmed. |
| event | `plan:changed` | — → `{ id, ts, projectId, revision }` | After every successful mutation. Not emitted for a failed one. |
| commands | `get_plan`, `plan_summary`, `add_slice`, `update_slice`, `remove_slice`, `set_dependencies`, `set_slice_status`, `add_decision`, `add_open_question`, `resolve_open_question`, `add_requirement`, `update_requirement`, `assign_requirement`, `remove_requirement`, `confirm_requirement`, `confirm_all`, `unconfirm_requirement`, `add_deferred`, `remove_deferred` | — → as in the bindings | Rules: `set_dependencies` (and `add_slice`) reject unknown ids, self-dependency and cycles. Confirming needs a slice. `queued → in_progress` needs every dependency done, or `options.override` with a non-empty `reason`, recorded as a decision ("Started before … done (override): reason"). Resolving a question records "question → resolution". Unassigning, or editing a confirmed requirement's text, kind or excerpt, makes it `proposed` again. `confirm_all` confirms every assigned requirement; unassigned ones stay proposed. New requirements get `unsourced` from `docs/brain-dump.md`, as in P3. |

`vsm_data.js` (repo root) is regenerated in the same locked operation as every successful mutation: save `plan.json`, then write the mirror. If the mirror can't be written, `plan.json` is restored and the mutation fails. The schema is taken from `vsm_viewer.html`, the file's actual reader, and is also shipped as `resources/templates/vsm_data.template.js` (decided by the human). It's deterministic: no revision and no timestamps; slices sorted by order, name, id; `depends_on` sorted; `future_horizon` sorted by label, id. `meta.title` / `subtitle` come from `.foundry/project.json` (name, tagline). It's never read back: a hand edit is overwritten by the next mutation.

Not in P2: P3's `generate_roadmap` doesn't load into this plan yet (the planned short follow-up). Requirement kinds are already aligned to the plan's (see the next section).

### P3 kinds aligned to the plan's (decided by the human)

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `Requirement.kind` (roadmap) | any non-empty string, prompt asking for `functional \| non-functional \| constraint` → still a `string` in TS, but always one of `behavior \| interaction \| constraint` | Matches `plan::RequirementKind`, so a generated roadmap loads into the plan as is. The prompt (`roadmap/prompt.md`) asks for these three; the validator (`roadmap/validate.rs`) rejects anything else, which triggers the usual retry with the allowed values in the error. The plan is unchanged. |

## P4 — Role Handbook, team composition, AGENTS.md and the Design Library

All additive, except `Settings` gaining `agentsTemplateId` (the UI must send it to `updateSettings`). Project commands take `projectId`. Nothing an LLM proposes reaches disk: `compose_team` is read-only; `confirm_team` and `add_agent_manually` are the only writers of `agents/`.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `Settings` | adds `agentsTemplateId: string` (default `"default"`; blank → default) | Which embedded AGENTS.md template `generate_agents` fills; only one exists. An unknown id → `invalid_input` from the placeholder and generate commands. |
| type | `ErrorCode` | adds `"unsupported_file" \| "team_confirmed" \| "team_not_confirmed" \| "placeholders_unconfirmed" \| "agents_md_exists"` | `unsupported_file`: non-`.md` into the Design Library. `team_confirmed`: `compose_team` / `confirm_team` on a project that already has a team (Law 6: re-proposing a confirmed team is out of scope; add with `add_agent_manually`). `team_not_confirmed`: adding or setting avatars before a team exists. |
| commands | `role_handbook_list`, `role_handbook_add(role)`, `role_handbook_update(id, patch)`, `role_handbook_remove(id)` | — → `HandbookRole[]` | App-level, in `<app data>/role-handbook/index.json`, seeded once with 12 starter roles (each with a real skill template and the protected floor). A deleted starter role isn't re-seeded. Ids are slugs of the title (`-2` on a clash). |
| command | `compose_team` | — → `composeTeam(projectId): TeamProposal` = `{ agents: [{ tempId, handbookRoleId \| null, customRole \| null, displayName, title, rationale, relatedSliceIds }] }` | `claude -p` (P3's runner, moved to `core::llm`) with the plan's slices and the handbook. One retry with the validation errors (unknown handbook role, both or neither of handbookRoleId/customRole, unknown slice, duplicate tempId, empty fields), then `generation_failed`. Writes nothing in any outcome, including to app data. |
| command | `confirm_team` | — → `confirmTeam(projectId, roster: RosterAgent[]): TeamWritten` | `RosterAgent` = `{ handbookRoleId \| customRole, displayName, title?, relatedSliceIds, avatarBuiltinId?, writeGlobs?, protectedGlobs? }`. Writes `agents/roles/<id>.md` (five headings: Reads, Writes, Hands off, Done means, When blocked) and `agents/roles.json` (`{ version, agents: TeamAgent[] }`, sorted by id). Agent ids are slugs of displayName (`-2` …), so identical rosters give identical bytes. `TeamWritten` = `{ team, created, floorRestored: [{ agentId, globs }] }`. |
| — | protected floor | every agent's `protectedGlobs` ⊇ `AGENTS.md`, `agents/**`, `docs/design/**`, `.foundry/plan.json`, `.claude/**` | Enforced on confirm and add. When the roster's or the handbook role's globs lacked any of these, it's restored and listed in `floorRestored`. |
| command | `add_agent_manually` | — → `addAgentManually(projectId, agent: RosterAgent): TeamWritten` | Adds one agent; existing entries and files are untouched. |
| command | `get_team` | — → `getTeam(projectId): Team \| null` | Beyond the spec: the UI needs to read the confirmed roster. |
| command | `builtin_avatar_list` | — → `string[]` | Beyond the spec: the built-in choices. Empty until images are added to `resources/avatars/` and registered (decided by the human: no generated placeholders). |
| command | `set_agent_avatar` | — → `setAgentAvatar(projectId, agentId, { builtinId \| bytes }): TeamAgent` | Upload: `core::image::process` (the Bakery's rules, moved to core), stored as `.foundry/agents/<agentId>-avatar.<png\|jpg>`. Built-in: `avatarPath = "builtin:<id>"`, no processing. |
| commands | `design_library_list`, `design_library_add_from_paths(paths)`, `design_library_update(id, { families, description })`, `design_library_remove(id)` | — → `DesignLibrary` = `{ files: DesignFile[], untaggedCount, families }` | App-level, `<app data>/design-library/<filename>` + `index.json`. `.md` only (and not `manifest.md`), all or nothing, unique filenames, 10 MB cap. Families are free-text tags, stored lowercase. |
| commands | `agents_placeholders(projectId)`, `confirm_placeholder(projectId, key, value)` | — → `Placeholder[]` = `{ key, label, hint, inTemplate, optional, value, confirmed, suggestions }` | The template's 7 markers (`inTemplate: true`; keys from their labels) plus `designFamily` (`inTemplate: false`, comma-separated tags, suggestions from the library), as decided by the human. Stored in `.foundry/agents-placeholders.json`. The optional marker accepts "" (its bullet is then removed). |
| command | `generate_agents` | — → `generateAgents(projectId, { overwrite }): GenerateAgentsResult` | Writes AGENTS.md (template minus its usage note and "(TEMPLATE)", decided by the human), copies matching library files byte-identical into `docs/design/`, writes `docs/design/manifest.md`, and removes earlier copies that no longer match (tracked in `.foundry/design-files.json`). Refuses on unconfirmed placeholders or a leftover marker. A differing AGENTS.md → `agents_md_exists` unless `overwrite`, which backs it up to `.foundry/backups/AGENTS.<ms>.md` first. No matching library file → `designCopySkipped: true` plus `unmatchedFamilies`, not an error. |

Template check: `resources/templates/AGENTS.template.md` Section 6 already routes through `docs/design/manifest.md`, has the Bakery paragraph, and ranks token file (3) → Bakery (4) → guideline files (5). Unchanged.

## P3's plan half: `load_roadmap` (decided by the human)

All additive. The plan feature stays the only writer of `plan.json`: it reads `.foundry/roadmap.json` from disk. The frontend calls `generate_roadmap`, then, on success, `load_roadmap`.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `load_roadmap` | — → `loadRoadmap(projectId, options: { discardConfirmed: boolean }, expectedRevision: number \| null): RoadmapLoaded` | Replaces the plan (a fresh proposal, not a merge) with the roadmap's slices (dependencies by name → ids), requirements (assigned and unassigned, `unsourced` kept, all `proposed`), deferred items and open questions. One mutation: one revision bump, one `vsm_data.js` write, one `plan:changed`. Any inconsistency (unknown kind, unknown dependency, cycle, duplicate slice name, unreadable file) fails the whole load and changes nothing. No roadmap → `not_found`. |
| type | `ErrorCode` | adds `"confirmed_requirements"` | `load_roadmap` while any requirement is confirmed and `discardConfirmed` is false (the spec's R-3-8 guard). |
| type | `RoadmapLoaded` | — → `{ plan, slices, requirements, deferred, openQuestions: number, unanchoredDeferred: string[] }` | Counts of what was loaded. `unanchoredDeferred`: labels of deferred items whose `anchoredTo` names no slice, so they couldn't be anchored and weren't loaded: reported, not dropped silently. Only `roadmap.json` files generated before `f039dc1` can still have these (anchored to a feature family); P3 now rejects any anchor that isn't a slice name. |
| type | `Plan` (and `plan.json`) | adds `openQuestions: OpenQuestion[]` | Project-wide open questions (the roadmap's aren't tied to a slice). Older `plan.json` files load with an empty list. Not mirrored into `vsm_data.js`, whose schema has no project-level slot. |

## Agent avatars for display: `agent_avatar`

Additive. Read-only: writes nothing to the repo or app data, and never changes the stored file.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `agent_avatar` | — → `agentAvatar(projectId: string, agentId: string): string \| null` | The UI needs to show avatars without asset-protocol access to `.foundry/agents/`. Returns a `data:` URL. Uploaded file (`avatarPath` under `.foundry/agents/`): resolved through `core::paths`, so a hand-edited `roles.json` can't point outside that folder (`path_escape`); returned at most 256 px on its long edge via `core::image::downscale`, in the stored format (`image/png` or `image/jpeg`), byte-identical when it already fits. `builtin:<id>`: the embedded bytes as `image/png`, unscaled (built-ins ship pre-sized). `null` when the agent has no avatar, the built-in id isn't embedded, or the file or folder is missing. Unknown project or agent → `not_found`; no confirmed team → `team_not_confirmed` (as `set_agent_avatar`). |

## App-wide settings fields (storage only)

`update_settings` still replaces the whole object, so the six new fields are a **contract change**: the frontend must now send them all (start from `get_settings` and change what's needed). Older `settings.json` files load with every default. No backend code reads these six fields; the frontend and later work do. `defaultModel` (`string | null`, `null`), `generationTimeoutMs` (`number`, `300000`) and `agentsTemplateId` (`"default"`, the one embedded template) already existed with these defaults, so they're unchanged.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `Settings` | adds `theme: Theme` (`"system"`), `reducedMotionOverride: boolean` (`false`), `autoInstallHooks: boolean` (`true`), `defaultVerifyMode: VerifyMode` (`"manual"`), `osNotificationsEnabled: boolean` (`true`), `osNotificationSeverity: NotificationSeverity` (`"danger"`) | App-wide preferences, stored in `<app data>/settings.json`. |
| type | `Theme` | — → `"light" \| "dark" \| "system"` | |
| type | `VerifyMode` | — → `"auto" \| "manual"` | |
| type | `NotificationSeverity` | — → `"danger" \| "warn_and_danger" \| "all"` | |
| command | `update_settings` | now also requires the six fields above; any other enum value → `invalid_input`, e.g. `theme must be one of "light", "dark", "system", not "neon"`, and nothing is saved | Rejected, never coerced. Each enum keeps a hidden catch-all variant (left out of the bindings), so a bad value reaches the validation and comes back as an `AppError`, not as a Tauri decode error. A bad value hand-edited into `settings.json` loads as that field's default; the rest of the file is kept. |

## Role Handbook import: `role_handbook_add_from_paths`

Additive command, type and error code. `compose_team` keeps its signature; what it sends the model changes (below).

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `role_handbook_add_from_paths` | — → `roleHandbookAddFromPaths(paths: string[], replace: boolean): RolesImported` | One handbook role per dropped `.md` file. Optional frontmatter (`title`, `description`, `writes`, `protected`; lists inline `[a, b]` or `- item` lines), parsed by hand; any other line → `invalid_input` naming the file and line. Body (BOM removed, LF) → `skillTemplate`. Title falls back to the first `# ` heading, then the file name without `.md`; description to the body's first non-heading paragraph, cut at 200 characters. `writes`/`protected` → `typicalWriteGlobs`/`typicalProtectedGlobs` (the floor is still added at `confirm_team`). Validated and given an id exactly as `role_handbook_add` (`slug(title)`, else `role`). `.md` only (`unsupported_file`), 100 KB per file (`file_too_large`), at most 50 files and two files with the same id → `invalid_input`. All or nothing; every refusal names the file. The files are only read. |
| type | `RolesImported` | — → `{ added: string[], replaced: string[] }` | Role ids. `replaced`: existing roles that kept their id (and place) and took the file's fields. |
| type | `ErrorCode` | adds `"role_exists"` | An imported role's id is already in the handbook and `replace` is false. Nothing changes; the message lists the existing titles. |
| behaviour | `compose_team` | the handbook sent to the model: every field of each role (`id, title, description, typicalWriteGlobs, typicalProtectedGlobs, skillTemplate`) → `id, title, description, typicalWriteGlobs` only | Imported skills can be long (up to 100 KB each); the model only needs to pick roles. Validation of the reply is unchanged. |
| behaviour | `confirm_team`, `add_agent_manually` | role files: the skill was embedded as written → its headings are demoted so the top one is level 3 (relative levels kept, capped at 6; setext headings become ATX; fenced code untouched) | A skill with its own headings must not add a heading at the level of the five required ones (Reads, Writes, Hands off, Done means, When blocked), which are unchanged. Skills without headings (all starter roles) give byte-identical files. |

## P6 — hand-off and task prompts

All additive. Prompts are assembled by code from the files on disk (`plan.json`, `roadmap.json`, `agents/roles.json`, `.foundry/agents-placeholders.json`, `.foundry/bakery.json`, `AGENTS.md`); no model is involved, and the same inputs always give the same bytes. Writes go only to `.foundry/prompts/` (already on the `repo_write` allowlist under `.foundry/`). `plan.json` is never written.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `ErrorCode` | adds `"plan_not_confirmed" \| "agents_md_not_ready"` | Gates for prompt generation: the plan isn't confirmed (`plan_summary.confirmed` is false), or AGENTS.md is missing or still has a `[[ FILL IN` marker. |
| command | `generate_handoff` | — → `generateHandoff(projectId): Result<HandoffResult, AppError>` where `HandoffResult = { path: string, warnings: string[] }` | Fills `resources/templates/handoff-prompt.template.md` into `.foundry/prompts/handoff.md`. Refused with `plan_not_confirmed`, `agents_md_not_ready`, `placeholders_unconfirmed` (architecture, frontend, backend or database value missing), `not_found` (the roadmap has no goal) or `invalid_input` (a `[[ FILL IN` would remain, e.g. inside a requirement). `warnings` lists unlabeled Bakery images; they don't refuse. |
| command | `generate_task_prompt` | — → `generateTaskPrompt(projectId, sliceId, agentId, options: TaskPromptOptions): Result<TaskPromptResult, AppError>` where `TaskPromptOptions = { force: boolean }`, `TaskPromptResult = { path: string, unfinishedDependencies: string[], warnings: string[] }` | Writes `.foundry/prompts/<sliceId>-<agentId>.md` in a fixed 8-part structure. Also needs a confirmed team (`team_not_confirmed`). Unknown slice or agent → `not_found`. Unfinished dependencies → `dependencies_not_done` unless `force`, which returns their names in `unfinishedDependencies` and adds a line to the prompt. The frontend records the decision through `add_decision`. |
| command | `launch_message` | — → `launchMessage(projectId, agentId, promptFile: string): Result<string, AppError>` | One line (no control characters, ≤ 500 chars): `You are {displayName}, {title}. Read agents/roles/{agentId}.md, then {promptFile} and follow it.` `promptFile` must be an existing file directly in `.foundry/prompts/` (resolved through `core::paths`), else `invalid_input`; it's returned normalized to forward slashes. A display name or title with a line break → `invalid_input`. For a task prompt whose slice has unfinished dependencies → `dependencies_not_done`, unless that prompt was generated with `force`. |
| command | `list_prompts` | — → `listPrompts(projectId): Result<PromptFile[], AppError>` where `PromptFile = { path: string, kind: PromptKind, sliceId: string \| null, agentId: string \| null, size: number, modifiedMs: number }`, `PromptKind = "handoff" \| "task"` | The prompts in `.foundry/prompts/`, sorted by path. Task file names are matched against the plan's slice ids (both ids may contain `-`); files that match neither kind are left out. |
| type | `SliceReport` (core, not yet in the bindings) | — → `{ sliceId, agent, requirements: { id, status: "met" \| "partly" \| "not_met", evidence }[], testsRun: { command, result }[], blockers: string[], notes: string }` | `core::report`: the shape agents write to `.foundry/reports/<sliceId>.json`. Task prompts show it by serializing this type, and P9 reads it with the same type, so the two can't drift. It joins the bindings when a command returns it. |
| command | `read_prompt` | — → `readPrompt(projectId, file: string): Result<string, AppError>` | The text of a prompt file as stored (UTF-8, unchanged), for showing it in the UI. `file` is a repo-relative `.md` file directly in `.foundry/prompts/` (`./` and `\` accepted), at most 200 KB (200,000 bytes). Read-only. Errors, all existing codes: unknown project or missing file (or a folder) → `not_found`; `..`, absolute path, any other folder or a subfolder, or a link out of the prompts folder → `path_escape`; control characters in the path → `invalid_path`; not `.md` → `unsupported_file`; over the cap → `file_too_large`; not UTF-8 → `invalid_input`. |

## P8a — agent events: classifier, event store, local hook server

All additive. The hook helper binary, the hooks installer and desktop notifications are P8b. Writes go only to `.foundry/events/` (on the `repo_write` allowlist under `.foundry/`), through `core::repo_write` (new `append` for the JSONL files; `read.json` is written atomically).

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| type | `AgentEvent` | — → `{ id: number, ts: number, projectId, agentId, sliceId: string \| null, kind: EventKind, severity: Severity, title, detail: string \| null, path: string \| null, command: string \| null }` | One thing an agent did. `id` is 1, 2, 3, … per project and agent, never reused (it continues across rotation and restarts). `path` is repo-relative with `/` when inside the repo. `detail` ≤ 300 characters, `command` ≤ 2000 (cut with `…`). |
| type | `EventKind` | — → `"session_started" \| "edit" \| "command" \| "tests_ran" \| "blocked" \| "needs_attention" \| "turn_complete" \| "session_ended" \| "stuck" \| "quiet" \| "danger" \| "report_ready"` | `danger` is reserved: no P8a rule produces it (blocked actions are kind `blocked`, severity `danger`). |
| type | `Severity` | — → `"info" \| "warn" \| "danger"` | Ordered `info < warn < danger` for `minSeverity`. |
| type | `AgentEventsQuery` | — → `{ beforeId: number \| null, limit: number \| null, minSeverity: Severity \| null }` | Missing keys read as null. `limit` 1–500, default 100; anything else → `invalid_input`. |
| type | `AgentEventCounts` | — → `{ agentId, unread: number, danger: number }` | `danger`: unread events with severity `danger`. |
| command | `agent_events` | — → `agentEvents(projectId: string, agentId: string, query: AgentEventsQuery): AgentEvent[]` | Newest first, `id < beforeId`, `severity >= minSeverity`, at most `limit`. Unknown project → `not_found` (missing folder → `folder_missing`); an `agentId` that isn't 1–64 of `[A-Za-z0-9_-]` → `invalid_input`. |
| command | `agent_event_counts` | — → `agentEventCounts(projectId: string): AgentEventCounts[]` | One entry per agent with an event file in the project, sorted by agent id. |
| command | `agent_mark_read` | — → `agentMarkRead(projectId: string, agentId: string, upToId: number): null` | Stored in `.foundry/events/read.json` (`{ agentId: upToId }`). Never moves back, and is clamped to the newest event, so later events still arrive unread. |
| event | `agent:event` | — → `AgentEvent` | Emitted after each event is stored. Carries `{ id, ts, ... }`; `id` is the event's number, not a UUID. |

Not in the Tauri contract, for P8b:

- **Files.** `.foundry/events/<agentId>.jsonl`, one `AgentEvent` per line. Before an append would take it past 5 MB it becomes `<agentId>.1.jsonl`, replacing the older one, so the last two files are kept. A torn last line (a crash mid-append) is skipped on read.
- **Hook HTTP interface.** `POST http://127.0.0.1:<port>/events`, header `x-foundry-token: <token>`, body `{ projectId, agentId, sliceId?, hookEvent, payload, decision?: { blocked, rule? } }`, at most 256 KB. Answered at once with `202 {"accepted":true}` and processed afterwards, in arrival order. Refusals are `{ code, message }`: `401 unauthorized` (missing or wrong token), `413 payload_too_large`, `400 bad_request` (not JSON, or a bad `agentId`), `404 unknown_project` (unknown `projectId` or a missing folder), `404 not_found` (any other route), `405 method_not_allowed`, `503 busy` (more than 1024 calls queued).
- **`core::hook`.** `HookEndpointCell` holds `HookEndpoint { port, token }` (`token` redacted from `Debug`), published once the server is bound and cleared when it stops. Managed as `Arc<HookEndpointCell>`, so agent sessions can read `FOUNDRY_HOOK_PORT` and `FOUNDRY_HOOK_TOKEN` without importing `agent_events`. Port and token are fresh every run (the token is 64 hex digits).
- **Classifier.** Edits and commands come from `PostToolUse` and `PostToolUseFailure`; `PreToolUse` gives an event only when `decision.blocked`. Edit tools are `Write`, `Edit`, `MultiEdit` and `NotebookEdit`; command tools are `Bash` and `PowerShell`; other tools give no event. Notifications of type `auth_success`, `elicitation_complete` and `elicitation_response` give no event.
- **Quiet.** Every 30 s a timer checks `core::sessions`. A `running` session with no accepted hook call for 10 minutes (counted from its last call or its start) gets one `quiet` event per stretch.

## P8b — foundry-hook helper, hooks installer, desktop notifications

The commands and types are additive. Two classifier changes alter what existing events say (marked **behaviour** below), because of real captured payloads.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `hooks_install` | — → `hooksInstall(projectId: string): HooksChange` | Merges Foundry's hook entries into `<repo>/.claude/settings.local.json` through `core::repo_write::merge_json`. The file is backed up first, to `.foundry/backups/settings.local.<ms>.json`. All user content is kept. It's idempotent: an unchanged file isn't rewritten or backed up. Refused, with nothing written: Claude Code missing → `claude_not_found`; older than 2.1.139, or its version unreadable → `claude_version_unsupported` (the message names the version and the minimum); no helper next to the app → `not_found`; the file isn't valid JSON, isn't an object, or its `hooks` isn't an object of arrays → `invalid_json`. |
| command | `hooks_uninstall` | — → `hooksUninstall(projectId: string): HooksChange` | Removes only Foundry's hooks: hooks whose `command` contains `foundry-hook`. A group, event list or `hooks` object that this empties is removed too, even an event list the user had left empty. Needs neither Claude Code nor the helper. Errors as `hooks_install` for invalid JSON. |
| command | `hooks_status` | — → `hooksStatus(projectId: string): HooksStatus` | Read-only. An invalid file reports `settingsValid: false`; it isn't an error. |
| type | `HooksStatus` | — → `{ installed: boolean, outdated: boolean, events: string[], helperPath: string, helperExists: boolean, settingsValid: boolean, gitIgnored: boolean \| null, claudeVersion: string \| null, claudeVersionSupported: boolean, minClaudeVersion: string }` | `gitIgnored` comes from `git check-ignore`, and is `null` outside a git repo. `claudeVersion` is the first line `claude --version` printed. |
| type | `HooksChange` | — → `{ status: HooksStatus, changed: boolean, backup: string \| null }` | |
| type | `ErrorCode` | adds `"invalid_json" \| "claude_version_unsupported"` | For the two refusals above. |
| behaviour | classifier: `PostToolUse` for a command | no exit code → "Tests ran" / "Ran a command" (outcome unknown) → passed ("Tests passed") | Real payloads have no exit code. A non-zero exit fires `PostToolUseFailure` instead. |
| behaviour | classifier: `PostToolUseFailure` | → `detail` is the payload's `error` (e.g. `Exit code 3`). `is_interrupt: true` → "Command interrupted" / "Tests interrupted", which doesn't count toward `stuck` | Real payload fields: `error` and `is_interrupt`, not the documented `tool_error`. |
| behaviour | classifier: `PreToolUse` allowed with a rule | no event → one `command` (or `edit`) event, severity `warn`, `detail` = the rule (`git_push`, `dependency_install`) | The helper's warnings show up in Foundry. |
| behaviour | desktop notifications | — → one OS notification (title `<agentId> · <project name>`, body = the event title) for an event at or above `osNotificationSeverity` (`danger` → danger; `warn_and_danger` → warn+; `all` → everything), only when `osNotificationsEnabled`. At most one per agent and project within 5 s: the most severe event of the burst. Settings are read on every event. | `tauri-plugin-notification` 2.5.0 (approved), registered in `run()`. |

Not in the Tauri contract:

- **Hook entries.** They use exec form: `{ "type": "command", "command": "<absolute path to foundry-hook(.exe)>", "args": ["hook"] }`. There's no shell, so a path with spaces is written verbatim and needs no quoting. Exec form arrived in Claude Code 2.1.139, per its CHANGELOG ("Added hook `args: string[]` field (exec form)"). Installed for `SessionStart`, `Notification`, `Stop`, `SessionEnd`, and, with matcher `Write|Edit|MultiEdit|NotebookEdit|Bash|PowerShell`, for `PreToolUse`, `PostToolUse` and `PostToolUseFailure`.
- **`foundry-hook` decisions** (`core::hook_policy`, shared with the binary through `#[path]`):
  - With no `FOUNDRY_AGENT_ID`, it does nothing and exits 0.
  - The repo root is `CLAUDE_PROJECT_DIR`, else the payload's `cwd`.
  - Edit tools are blocked (exit 2, stderr names the rule and says to ask the human) for: a missing `roles.json` (`roles_missing`); one that doesn't parse (`roles_invalid`); an agent not in it (`unknown_agent`); no file named (`no_target_path`); a `..` (`path_escape`); a path outside the repo after resolving links (`outside_repo`); the protected floor or the agent's `protectedGlobs` (`protected_path`); anything else outside `writeGlobs` (`outside_write_globs`).
  - Every agent may write `.foundry/reports/<name>.json` (a direct child) unless it's protected.
  - Bash and PowerShell commands are blocked for `git_push_force`, `git_reset_hard`, `git_clean_force` (any force without `-n`) and `rm_rf_outside`. A `git_push` or `dependency_install` is allowed with a warning: exit 0 and `{ systemMessage, hookSpecificOutput.additionalContext }`, never `permissionDecision`.
  - Unreadable stdin → exit 2.
- **The helper's POST.** It goes to `127.0.0.1:FOUNDRY_HOOK_PORT/events` in P8a's body shape, with `sliceId` from `FOUNDRY_SLICE_ID` and `decision: { blocked, rule }` on judged `PreToolUse` calls. It needs `FOUNDRY_PROJECT_ID`, `FOUNDRY_HOOK_PORT` and `FOUNDRY_HOOK_TOKEN`. File contents, diffs and command output are dropped from `tool_input`/`tool_response` first, and an over-cap body falls back to the fields the classifier reads. Connect, send and the first bytes of the answer must all fit in 500 ms, and the answer is ignored.
- **`core::repo_write::merge_json`.** The only writer for merge-only files. A missing file reads as `{}`. Invalid JSON or a non-object → `invalid_json`, untouched. A merge that changes nothing writes nothing. A UTF-8 BOM is kept. Keys come out sorted: serde_json's `preserve_order` isn't enabled.
- **Sidecar.** `tauri.bundle.conf.json` adds `bundle.externalBin: ["binaries/foundry-hook"]`, and its `beforeBuildCommand` runs `scripts/stage-foundry-hook.mjs`, which builds and copies the helper to `binaries/foundry-hook-<triple>(.exe)` (git-ignored). Bundle with `tauri build --config src-tauri/tauri.bundle.conf.json`. In dev, `beforeDevCommand` builds `foundry-hook` first. Either way the app finds it next to its own executable (`core::hook::helper_path`).

## agent_start wired; sessions keyed by project (decided by the human)

**Breaking**, approved by the human: no UI used these commands yet. A session is identified by `(projectId, agentId)`, because agent ids are slugs of display names and two projects can have the same one.

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `agent_list` | `agentList(): SessionInfo[]` (every session) → `agentList(projectId): SessionInfo[]` | That project's live sessions, ordered by agent id. |
| command | `agent_start` | `agentStart(agentId, options): SessionInfo`, always `not_implemented` → `agentStart(projectId, agentId, options): SessionInfo` | Real launch (below). |
| command | `agent_write` | `agentWrite(agentId, data)` → `agentWrite(projectId, agentId, data)` | `not_found` unless that agent has a running session in that project. |
| command | `agent_resize` | `agentResize(agentId, cols, rows)` → `agentResize(projectId, agentId, cols, rows)` | |
| command | `agent_stop` | `agentStop(agentId, options)` → `agentStop(projectId, agentId, options)` | Stopping one project's session leaves the same agent id in another project running. |
| command | `agent_snapshot` | `agentSnapshot(agentId)` → `agentSnapshot(projectId, agentId)` | Empty if the agent hasn't run in that project. |
| event | `agent:output` | `{ id, ts, agentId, chunk }` → `{ id, ts, projectId, agentId, chunk }` | |
| event | `agent:status` | `{ id, ts, agentId, status, exitCode, pid, since }` → `{ id, ts, projectId, agentId, status, exitCode, pid, since }` | |
| type | `AgentStartOptions` | `{ promptFile: string \| null, sliceId: string \| null }` → `{ promptFile: string, sliceId?: string \| null }` | A prompt file is required; `sliceId` may be left out. |
| type | `ErrorCode` | removes `"not_implemented"` | Only the `agent_start` stub returned it. |

Unchanged: `SessionInfo` already carried `projectId`. The cap (`maxConcurrentAgents`) still counts sessions in every project; one session per agent now means per agent per project (`session_running`).

`agent_start(projectId, agentId, { promptFile, sliceId? })`, in order, nothing spawned unless all pass:

1. The project's repo: `not_found`, `folder_missing`.
2. A confirmed team (`agents/roles.json` exists): `team_not_confirmed`. The agent is in it: `not_found`.
3. `promptFile` is an existing file directly in `.foundry/prompts/`, resolved through `core::paths` (`./` and `\` accepted; a link out of the folder doesn't count): `invalid_input` otherwise, missing included. Same rule as `launch_message`.
4. `sliceId`, when given, is 1–200 characters on one line: `invalid_input`.
5. The launch message is built by the backend, never taken from the frontend: `You are {displayName}, {title}. Read agents/roles/{agentId}.md, then {promptFile} and follow it.`, one line, no control characters, at most 500 characters (`invalid_input`).
6. The hook server's port and token from `core::hook::HookEndpointCell`: `internal` if the server isn't running.
7. `SessionManager::launch` as before: preflight with `claudePath` (`claude_not_found` / `not_signed_in` / `wrong_auth_mode`), `claude_needs_native` for a `.cmd`/`.bat` shim, `project_archived` (atomic with the reservation), then `session_running` / `too_many_sessions`. Runs `claude [--model <defaultModel>] "<message>"`.

Slice dependencies aren't re-checked: `generate_task_prompt` gates them. The terminal opens at 120×30; the view sends its size with `agent_resize`. The environment is unchanged except that `FOUNDRY_SLICE_ID=<sliceId>` is added only when a slice is given; an inherited `FOUNDRY_SLICE_ID` is always dropped.

Internal: the launch-message builder and its checks moved from the prompts feature to `core::launch_message` (`roster`, `find_agent`, `prompt_file`, `build`). `launch_message` calls it, with the same output and errors. `core::sessions::SessionRegistry` is keyed by `SessionKey { projectId, agentId }` and gains `list_project`. `SessionManager::with_launcher` takes the `Arc<HookEndpointCell>`. fake-claude's interactive mode acts on `FAKE:ENV` / `FAKE:ARGS` anywhere in the message.
