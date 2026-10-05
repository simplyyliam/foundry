import { useRef, useState } from "react";
import type { Settings } from "@/lib/bindings";
import { saveSetting } from "./settingsStore";

export interface SettingField<K extends keyof Settings> {
  /** What the control shows: the value being saved, else the saved one. */
  value: Settings[K];
  saving: boolean;
  /** The backend's message from the last failed save, as-is. */
  error: string | null;
  /** Saves `next`. On failure the control goes back to the saved value and `error` is set. */
  commit: (next: Settings[K]) => Promise<boolean>;
}

/** One control's view of one settings field. `settings` is the latest saved copy. */
export function useSettingField<K extends keyof Settings>(settings: Settings, key: K): SettingField<K> {
  const [pending, setPending] = useState<{ value: Settings[K] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Only the newest save clears `pending`, so quick toggles don't flicker back.
  const latest = useRef(0);

  const commit = async (next: Settings[K]) => {
    if (pending === null && Object.is(next, settings[key])) {
      setError(null);
      return true;
    }
    const run = ++latest.current;
    setPending({ value: next });
    setError(null);
    const outcome = await saveSetting(key, next);
    if (run !== latest.current) return outcome.ok;
    setPending(null);
    if (!outcome.ok) setError(outcome.error.message);
    return outcome.ok;
  };

  return {
    value: pending ? pending.value : settings[key],
    saving: pending !== null,
    error,
    commit,
  };
}
