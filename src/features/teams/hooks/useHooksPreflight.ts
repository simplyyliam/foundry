import { useCallback } from "react";
import { commands, type HooksChange, type HooksStatus, type Settings } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export interface HooksCheck {
  hooks: Outcome<HooksStatus>;
  settings: Outcome<Settings>;
}

export interface HooksPreflight {
  /** `hooksStatus` and `getSettings` together, read fresh before each start. */
  check: () => Promise<HooksCheck>;
  install: () => Promise<Outcome<HooksChange>>;
}

/** What the start dialog needs to know (and do) about hooks before a session starts. */
export function useHooksPreflight(projectId: string): HooksPreflight {
  const check = useCallback(async () => {
    const [hooks, settings] = await Promise.all([
      call(() => commands.hooksStatus(projectId)),
      call(() => commands.getSettings()),
    ]);
    return { hooks, settings };
  }, [projectId]);

  const install = useCallback(() => call(() => commands.hooksInstall(projectId)), [projectId]);

  return { check, install };
}
