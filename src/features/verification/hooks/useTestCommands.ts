import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type TestCommands } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type TestCommandsLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; value: TestCommands };

/**
 * The project's test commands and whether they're the confirmed list. There's no event for
 * them changing (an agent can edit `.foundry/project.json`), so they're read on mount, after
 * every save or confirm, and when the caller asks (e.g. after `test_commands_unconfirmed`).
 */
export function useTestCommands(projectId: string) {
  const [load, setLoad] = useState<TestCommandsLoad>({ status: "loading" });
  const loaded = useRef(false);
  const latestFetch = useRef(0);

  const adopt = useCallback((value: TestCommands) => {
    loaded.current = true;
    setLoad({ status: "ready", value });
  }, []);

  const reload = useCallback(async () => {
    const request = ++latestFetch.current;
    const outcome = await call(() => commands.getTestCommands(projectId));
    if (request !== latestFetch.current) return;
    if (outcome.ok) adopt(outcome.data);
    else if (!loaded.current) setLoad({ status: "error", error: outcome.error });
  }, [projectId, adopt]);

  useEffect(() => {
    void reload();
    const fetches = latestFetch;
    return () => {
      fetches.current++;
    };
  }, [reload]);

  const save = useCallback(
    async (list: string[]): Promise<Outcome<TestCommands>> => {
      const outcome = await call(() => commands.setTestCommands(projectId, list));
      if (outcome.ok) {
        latestFetch.current++;
        adopt(outcome.data);
      }
      return outcome;
    },
    [projectId, adopt],
  );

  /**
   * Confirms exactly `shown`, the list on screen. `test_commands_changed` (it changed on disk
   * meanwhile) confirms nothing; the list is read again so the new one shows.
   */
  const confirm = useCallback(
    async (shown: string[]): Promise<Outcome<TestCommands>> => {
      const outcome = await call(() => commands.confirmTestCommands(projectId, shown));
      if (outcome.ok) {
        latestFetch.current++;
        adopt(outcome.data);
      } else if (outcome.error.code === "test_commands_changed") {
        await reload();
      }
      return outcome;
    },
    [projectId, adopt, reload],
  );

  return { load, reload, save, confirm };
}
