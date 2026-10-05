import { useEffect, useSyncExternalStore } from "react";
import { commands, type Settings } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type SettingsState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; settings: Settings };

/**
 * One app-wide copy of the saved settings. Mounted from app start (theme, reduced motion)
 * and read by the Settings dialog.
 */
let state: SettingsState = { status: "loading" };
const listeners = new Set<() => void>();
/** Loads and saves run one at a time, in order, so a quick second change can't overwrite the first. */
let queue: Promise<unknown> = Promise.resolve();
let started = false;

function setState(next: SettingsState) {
  state = next;
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function enqueue<T>(task: () => Promise<T>): Promise<T> {
  const run = queue.then(task);
  queue = run.catch(() => undefined);
  return run;
}

/** Re-reads the settings (after any queued saves). A failed reload keeps what we have. */
export function reloadSettings(): Promise<void> {
  return enqueue(async () => {
    const outcome = await call(() => commands.getSettings());
    if (outcome.ok) setState({ status: "ready", settings: outcome.data });
    else if (state.status !== "ready") setState({ status: "error", message: outcome.error.message });
  });
}

/**
 * Saves one field: `updateSettings({ ...latest, [key]: value })`, where `latest` is the most
 * recently saved settings at the moment this save runs (not when it was asked for).
 */
export function saveSetting<K extends keyof Settings>(key: K, value: Settings[K]): Promise<Outcome<Settings>> {
  return enqueue(async () => {
    if (state.status !== "ready") {
      return { ok: false, error: { code: "internal", message: "Settings haven't loaded yet." } } as Outcome<Settings>;
    }
    const latest = state.settings;
    const outcome = await call(() => commands.updateSettings({ ...latest, [key]: value }));
    if (outcome.ok) setState({ status: "ready", settings: outcome.data });
    return outcome;
  });
}

/** The saved settings; loads them on first use. */
export function useSettingsState(): SettingsState {
  useEffect(() => {
    if (started) return;
    started = true;
    void reloadSettings();
  }, []);
  return useSyncExternalStore(subscribe, () => state);
}
