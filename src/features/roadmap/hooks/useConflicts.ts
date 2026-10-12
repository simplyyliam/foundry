import { useCallback, useEffect, useSyncExternalStore } from "react";
import {
  commands,
  events,
  type AppError,
  type ConflictResolved,
  type ConflictRuling,
  type RoadmapConflict,
} from "@/lib/bindings";
import { call, type Outcome } from "./call";

/**
 * Per-project cache of the plan's roadmap conflicts, shared by the panel and the pop-out.
 * Read with `listRoadmapConflicts` plus `getPlan` (only for its revision, which rulings send
 * as `expectedRevision`), and again on `plan:changed` for a revision we don't hold yet.
 * Rulings run one at a time and adopt the conflict and revision they return.
 * `stale_revision` re-reads, like every plan edit.
 */

export interface ConflictsData {
  status: "loading" | "ready" | "error";
  conflicts: RoadmapConflict[];
  /** The plan revision the list was read at (or a ruling returned). */
  revision: number | null;
  error: AppError | null;
}

const LOADING: ConflictsData = { status: "loading", conflicts: [], revision: null, error: null };

const entries = new Map<string, ConflictsData>();
const latestRequest = new Map<string, number>();
const queues = new Map<string, Promise<unknown>>();
const listeners = new Set<() => void>();

function publish(projectId: string, data: ConflictsData) {
  entries.set(projectId, data);
  listeners.forEach((listener) => listener());
}

export async function refetchConflicts(projectId: string): Promise<void> {
  const request = (latestRequest.get(projectId) ?? 0) + 1;
  latestRequest.set(projectId, request);
  const [list, plan] = await Promise.all([
    call(() => commands.listRoadmapConflicts(projectId)),
    call(() => commands.getPlan(projectId)),
  ]);
  if (latestRequest.get(projectId) !== request) return;
  const previous = entries.get(projectId) ?? LOADING;
  if (list.ok) {
    publish(projectId, {
      status: "ready",
      conflicts: list.data,
      revision: plan.ok ? plan.data.revision : previous.revision,
      error: null,
    });
  } else {
    // A failed refresh keeps showing what was there.
    publish(projectId, { ...previous, status: previous.status === "ready" ? "ready" : "error", error: list.error });
  }
}

function adopt(projectId: string, resolved: ConflictResolved) {
  // Supersedes any read still in flight.
  latestRequest.set(projectId, (latestRequest.get(projectId) ?? 0) + 1);
  const previous = entries.get(projectId) ?? LOADING;
  publish(projectId, {
    status: "ready",
    conflicts: previous.conflicts.map((conflict) => (conflict.id === resolved.conflict.id ? resolved.conflict : conflict)),
    revision: resolved.plan.revision,
    error: null,
  });
}

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

function mutate(projectId: string, run: (revision: number | null) => Promise<Result<ConflictResolved>>) {
  const previous = queues.get(projectId) ?? Promise.resolve();
  const next = previous.then(async (): Promise<Outcome<ConflictResolved>> => {
    const outcome = await call(() => run(entries.get(projectId)?.revision ?? null));
    if (outcome.ok) adopt(projectId, outcome.data);
    else if (outcome.error.code === "stale_revision") await refetchConflicts(projectId);
    return outcome;
  });
  queues.set(projectId, next.catch(() => {}));
  return next;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useConflicts(projectId: string) {
  const data = useSyncExternalStore(subscribe, () => entries.get(projectId) ?? LOADING);

  useEffect(() => {
    void refetchConflicts(projectId);
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.planChanged
      .listen(({ payload }) => {
        if (payload.projectId !== projectId) return;
        // Our own ruling already adopted this revision.
        if (payload.revision === entries.get(projectId)?.revision) return;
        void refetchConflicts(projectId);
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [projectId]);

  const resolve = useCallback(
    (conflictId: string, ruling: ConflictRuling, addAsRule: boolean) =>
      mutate(projectId, (revision) => commands.resolveRoadmapConflict(projectId, conflictId, ruling, addAsRule, revision)),
    [projectId],
  );
  const unresolve = useCallback(
    (conflictId: string) => mutate(projectId, (revision) => commands.unresolveRoadmapConflict(projectId, conflictId, revision)),
    [projectId],
  );

  return { data, resolve, unresolve };
}

export type ConflictsController = ReturnType<typeof useConflicts>;

/** Tests only: forget every project's conflicts. */
export function resetConflicts() {
  entries.clear();
  latestRequest.clear();
  queues.clear();
}
