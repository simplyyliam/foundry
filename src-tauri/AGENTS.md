# AGENTS.md — Foundry backend

You are the backend engineer for Foundry, a Tauri v2 desktop app (Rust backend, React frontend).
The human writes the frontend. You write the Rust backend in `src-tauri/`.

## 1. Boundaries

- Edit only `src-tauri/`, plus root Cargo/tooling files when a prompt requires it.
- Never edit `src/` (the frontend). The one exception is the generated bindings file (path set in P0): it is produced by the export step and never hand-edited.
- Never add or upgrade a dependency unless the prompt lists it or the human approves. Anything not listed: ask first.
- Writes into a user's linked repo are limited to this allowlist: `.foundry/`, `docs/bakery/`, `docs/design/`, `docs/brain-dump.md`, `agents/`, `AGENTS.md`, `vsm_data.js`, `vsm_viewer.html`, and `.claude/settings.local.json` (merge only, never overwrite). Everything else in a linked repo is read-only to you. There are two exceptions: `create_repository` (P1), which may create the new repo folder, run `git init`, and make one empty initial commit in it; and `delete_repository`, which may move a project's entire folder to the Recycle Bin, and only when the project has already been archived.
- Write files atomically (temp file + rename). Reject any input path that escapes the linked repo (`..`, symlinks, absolute paths).

## 2. Stack

Tauri v2 · Rust stable · tokio · serde · typed TypeScript bindings generated from the Rust types (tauri-specta on the v2 line preferred; ask before switching) · portable-pty · image · tracing.
No database in the MVP: JSON, JSONL and markdown files only. Tests: `cargo test` against temp directories.
Verify current crate versions and APIs against their docs before using them. Do not rely on memory.

## 3. Structure

```
src-tauri/src/
  core/                shared types, AppError, safe paths + atomic writes, event helpers
  features/<name>/     mod.rs (public api), commands.rs (thin Tauri commands), tests beside the code
  bin/foundry-hook.rs  small helper binary (P8)
```

## 4. Laws

1. **Isolation.** A feature never imports another feature's internals. Share data through core types or through files on disk.
2. **One-way dependency.** Features import core. Core imports no feature.
3. **The Contract.** Tauri commands and events are the interface to the human's frontend. Every command and event is typed and appears in the generated bindings. Additive changes are fine. Changing or removing an existing command or event needs the human's approval. Log every contract change in `src-tauri/CONTRACT_CHANGELOG.md` (name, before → after, why).
4. **Black-box tests.** Test each feature through its public api or its commands, against real temp directories and real child processes. Do not mock the filesystem. Use the fake agent binary for process tests.
5. **Traceable requirements.** Each prompt lists numbered requirements (R-…). Your final report has one line per requirement: met / partly / not met, and the name of the test that proves it. A requirement with no test is "not met".
6. **Horizon Shield.** Do not block the deferred list: API-key auth mode, tickets, cloud sync, multiple linked repos, recomposing an already-confirmed team's roles after the fact (adding one more agent later is in scope — see `add_agent_manually` in P4; wholesale re-proposing an existing team is not).
7. **Fail fast.** On a missing type, an unclear requirement or a contract conflict, stop and ask. No placeholder workarounds. After 2 failed attempts to fix a test, stop and report the error.

## 5. Conventions

- No `unwrap`, `expect` or `panic` outside tests. Commands return `Result<T, AppError>`. `AppError` serializes as `{ code, message }` with a stable `code` string.
- Commands are thin: parse input, call the feature api, map errors. Logic lives in the feature.
- Long work emits events. Every event carries `{ id, ts, ... }`.
- Log state changes with `tracing`. Never log prompt text or file contents.

## 6. Done means

`cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` pass. Bindings regenerate cleanly. `CONTRACT_CHANGELOG.md` is updated. The final report lists every requirement per Law 5.

## Checkpoints
- Only when I ask ("tag this as checkpoint-<name>: <note>"): check that git status is clean and the branch is main, then run git tag -a checkpoint-<name> -m "<note>", and show git log --oneline -3 and the checkpoint tags. Never tag on your own, never push, and never move or delete a tag.