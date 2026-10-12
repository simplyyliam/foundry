import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type CommitSetupResult, type SetupStatus } from "@/lib/bindings";
import { useTeamChanged } from "@/shared";
import { call, type Outcome } from "./call";
import { useSetupFilesWritten } from "./setupFiles";

export type SetupStatusLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; value: SetupStatus };

export interface SetupCommit {
  load: SetupStatusLoad;
  reload: () => void;
  committing: boolean;
  /** The last failed commit, as the backend said it (e.g. `git_identity_missing` and how to fix it). */
  error: AppError | null;
  /** The last successful commit, until dismissed or another commit starts. */
  result: CommitSetupResult | null;
  commit: () => Promise<Outcome<CommitSetupResult>>;
  dismissResult: () => void;
}

/**
 * Which setup files `commitSetup` would commit (`setupStatus`), and committing them. There's no
 * event for setup files changing, so the status is read again: after AGENTS.md is generated,
 * when the team changes, on `plan:changed` (`.foundry/plan.json` is one of them), when the
 * window gets focus (a commit made in a terminal) and after a commit.
 */
export function useSetupCommit(projectId: string): SetupCommit {
  const [load, setLoad] = useState<SetupStatusLoad>({ status: "loading" });
  const [committing, setCommitting] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [result, setResult] = useState<CommitSetupResult | null>(null);
  const latestFetch = useRef(0);
  const mounted = useRef(true);

  const reload = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.setupStatus(projectId)).then((outcome) => {
      if (!mounted.current || request !== latestFetch.current) return;
      if (outcome.ok) setLoad({ status: "ready", value: outcome.data });
      // A failed re-read keeps what's shown; only a failed first load replaces it.
      else setLoad((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
  }, [projectId]);

  useEffect(() => {
    mounted.current = true;
    reload();
    return () => {
      mounted.current = false;
    };
  }, [reload]);

  useSetupFilesWritten(projectId, reload);
  useTeamChanged(projectId, reload);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.planChanged
      .listen(({ payload }) => payload.projectId === projectId && reload())
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    window.addEventListener("focus", reload);
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", reload);
    };
  }, [projectId, reload]);

  const commit = useCallback(async () => {
    setCommitting(true);
    setError(null);
    setResult(null);
    const outcome = await call(() => commands.commitSetup(projectId));
    if (!mounted.current) return outcome;
    setCommitting(false);
    if (outcome.ok) setResult(outcome.data);
    else setError(outcome.error);
    reload();
    return outcome;
  }, [projectId, reload]);

  const dismissResult = useCallback(() => setResult(null), []);

  return { load, reload, committing, error, result, commit, dismissResult };
}
