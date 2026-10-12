import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type GenerateAgentsResult, type Placeholder } from "@/lib/bindings";
import { useSettingsDialog } from "@/shared";
import { call, type Outcome } from "./call";
import { notifySetupFilesWritten } from "./setupFiles";

export type PlaceholdersLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; placeholders: Placeholder[] };

export interface AgentsSetup {
  load: PlaceholdersLoad;
  /**
   * The backend's message once any call answered `folder_missing` (the project's folder was
   * moved or deleted). Everything is disabled while it's set.
   */
  folderMissing: string | null;
  reload: () => Promise<void>;
  /** Confirms (or re-confirms with a new value) one placeholder; adopts the list it returns. */
  confirm: (key: string, value: string) => Promise<Outcome<Placeholder[]>>;
  /** Writes AGENTS.md and the design copies. `agents_md_exists` unless `overwrite`. */
  generate: (overwrite: boolean) => Promise<Outcome<GenerateAgentsResult>>;
}

/**
 * The project's AGENTS.md placeholders, and generating the file. Calls run one at a time, in
 * order. The list is read again when the Settings dialog closes, since the design-family
 * suggestions come from the Design Library and there's no event for it changing.
 */
export function useAgentsSetup(projectId: string): AgentsSetup {
  const [load, setLoad] = useState<PlaceholdersLoad>({ status: "loading" });
  const [folderMissing, setFolderMissing] = useState<string | null>(null);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const mounted = useRef(true);

  const enqueue = useCallback(<T,>(task: () => Promise<T>): Promise<T> => {
    const run = queue.current.then(task);
    queue.current = run.catch(() => undefined);
    return run;
  }, []);

  /** Notes `folder_missing` from any call, then hands the outcome back. */
  const watch = useCallback(<T,>(outcome: Outcome<T>): Outcome<T> => {
    if (mounted.current && !outcome.ok && outcome.error.code === "folder_missing") setFolderMissing(outcome.error.message);
    return outcome;
  }, []);

  const fetchList = useCallback(async () => {
    const outcome = watch(await call(() => commands.agentsPlaceholders(projectId)));
    if (!mounted.current) return;
    if (outcome.ok) {
      setLoad({ status: "ready", placeholders: outcome.data });
      setFolderMissing(null);
    } else {
      // A failed re-read keeps the list we have; only the first load shows the error.
      setLoad((current) => (current.status === "ready" ? current : { status: "error", error: outcome.error }));
    }
  }, [projectId, watch]);

  const reload = useCallback(() => enqueue(fetchList), [enqueue, fetchList]);

  useEffect(() => {
    mounted.current = true;
    void reload();
    return () => {
      mounted.current = false;
    };
  }, [reload]);

  const { open: settingsOpen } = useSettingsDialog();
  const settingsWasOpen = useRef(settingsOpen);
  useEffect(() => {
    if (settingsWasOpen.current && !settingsOpen) void reload();
    settingsWasOpen.current = settingsOpen;
  }, [settingsOpen, reload]);

  const confirm = useCallback(
    (key: string, value: string) =>
      enqueue(async () => {
        const outcome = watch(await call(() => commands.confirmPlaceholder(projectId, key, value)));
        if (outcome.ok && mounted.current) setLoad({ status: "ready", placeholders: outcome.data });
        return outcome;
      }),
    [enqueue, projectId, watch],
  );

  const generate = useCallback(
    (overwrite: boolean) =>
      enqueue(async () => {
        const outcome = watch(await call(() => commands.generateAgents(projectId, { overwrite })));
        // AGENTS.md and the design copies are setup files: the commit section reads its status again.
        if (outcome.ok) notifySetupFilesWritten(projectId);
        return outcome;
      }),
    [enqueue, projectId, watch],
  );

  return { load, folderMissing, reload, confirm, generate };
}
