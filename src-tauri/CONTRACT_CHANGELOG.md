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

All additive. Loading the result into the plan as `proposed`, and the `discardConfirmed` guard, wait for the plan store (P2). Adding `discardConfirmed` to `generate_roadmap` then will be a signature change, for approval.

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
