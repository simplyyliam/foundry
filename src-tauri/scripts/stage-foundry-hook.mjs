// Builds the foundry-hook helper and stages it where Tauri's `bundle.externalBin` expects a
// sidecar: src-tauri/binaries/foundry-hook-<target triple>[.exe]. Run by the bundle config's
// beforeBuildCommand (tauri build --config src-tauri/tauri.bundle.conf.json).
//
//   node src-tauri/scripts/stage-foundry-hook.mjs [--debug]
//
// TAURI_CONFIG is removed for the inner cargo build: with the bundle config merged in, the
// app's build script would look for the sidecar this script is about to create.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const srcTauri = dirname(dirname(fileURLToPath(import.meta.url)));
const debug = process.argv.includes("--debug");
const triple =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  execFileSync("rustc", ["--print", "host-tuple"], { encoding: "utf8" }).trim();
const host = execFileSync("rustc", ["--print", "host-tuple"], { encoding: "utf8" }).trim();
const exe = triple.includes("windows") ? ".exe" : "";

const env = { ...process.env };
delete env.TAURI_CONFIG;
const args = ["build", "--bin", "foundry-hook", "--manifest-path", join(srcTauri, "Cargo.toml")];
if (!debug) args.push("--release");
if (triple !== host) args.push("--target", triple);
execFileSync("cargo", args, { stdio: "inherit", env });

const targetDir = env.CARGO_TARGET_DIR || join(srcTauri, "target");
const profileDir = join(targetDir, triple !== host ? triple : "", debug ? "debug" : "release");
const from = join(profileDir, `foundry-hook${exe}`);
const to = join(srcTauri, "binaries", `foundry-hook-${triple}${exe}`);
mkdirSync(dirname(to), { recursive: true });
copyFileSync(from, to);
console.log(`staged ${from} -> ${to}`);
