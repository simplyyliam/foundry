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
| command | `archive_project` | — → `archiveProject(id): Project` | Sets `archived = true`. Registry only; idempotent. `not_found` for an unknown id. If it's the active project it's also closed (`activeProject()` → null, same as no project open); no event is emitted. Follow-up for P7: refuse while the project has a running agent session (not enforceable yet: no session state exists). |
| command | `unarchive_project` | — → `unarchiveProject(id): Project` | Sets `archived = false`. This is the restore action (nothing left disk). Idempotent. `not_found` for an unknown id. |
| command | `remove_project` | `removeProject(id): null` (any project) → `removeProject(id): null`, **archived projects only**; otherwise `not_archived` and nothing changes | The backend enforces "you can only delete what you've already archived". Still unregisters only and never deletes repo files. Chosen instead of adding a new `delete_project` (the human decided). |

## Delete to Recycle Bin

Additive: one new command. It replaces R-1-5 ("removing never deletes repo files") **for this command only**. `remove_project` is unchanged: it still only unregisters, archived projects only (test: `remove_project_still_only_unregisters_alongside_delete_repository`).

| Kind | Name | Before → After | Why |
| --- | --- | --- | --- |
| command | `delete_repository` | — → `deleteRepository(id): Result<null, AppError>` | Moves an archived project's folder to the Recycle Bin, then unregisters it and clears the active project if it was this one. Folder already gone → just unregisters. Errors: `not_found` (unknown id), `not_archived` (not archived; nothing changes), `io` if the Recycle Bin move fails, e.g. a file is open elsewhere: `could not move <path> to the Recycle Bin: a file in it is open in another program; close it and try again (0x80270028)`. Other failures carry the OS's message and HRESULT. On `io` the folder and the registration are untouched. If Windows can't recycle the folder (e.g. too big for the bin), it shows its own "delete permanently?" prompt instead of deleting silently; declining → `io` "…was not moved to the Recycle Bin (cancelled)". Windows only for now: other OSes always get `io` "…isn't supported on this OS yet". Uses the existing `windows` 0.62 dependency with `Win32_System_Com` and `Win32_UI_Shell` turned on (no new crate). |
