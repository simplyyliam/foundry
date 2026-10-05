import { useEffect, useSyncExternalStore } from "react";
import { commands, type AppError, type BakeryAddResult, type BakeryPatch, type BakeryState } from "@/lib/bindings";
import { call, type Outcome } from "./call";

/**
 * Per-project cache of the Bakery, shared by the panel and its pop-out (both can be mounted
 * at once). Read with `bakeryList` on mount and after every change. Changes run one at a
 * time, so a label rename (which renames the file) never races the next edit.
 * `bakeryMove` and drop points are never used: this view has no canvas layout.
 */

export interface BakeryData {
  /** "loading" only until the first response for this project. */
  status: "loading" | "ready" | "error";
  bakery: BakeryState | null;
  /** The last failed `bakeryList`, as-is. */
  error: AppError | null;
}

const LOADING: BakeryData = { status: "loading", bakery: null, error: null };

const entries = new Map<string, BakeryData>();
const latestRequest = new Map<string, number>();
const queues = new Map<string, Promise<unknown>>();
const listeners = new Set<() => void>();

function publish(projectId: string, data: BakeryData) {
  entries.set(projectId, data);
  listeners.forEach((listener) => listener());
}

function adopt(projectId: string, bakery: BakeryState) {
  // A newer response supersedes any list still in flight.
  latestRequest.set(projectId, (latestRequest.get(projectId) ?? 0) + 1);
  publish(projectId, { status: "ready", bakery, error: null });
}

export async function refetchBakery(projectId: string): Promise<void> {
  const request = (latestRequest.get(projectId) ?? 0) + 1;
  latestRequest.set(projectId, request);
  const outcome = await call(() => commands.bakeryList(projectId));
  if (latestRequest.get(projectId) !== request) return;
  const previous = entries.get(projectId) ?? LOADING;
  publish(
    projectId,
    outcome.ok
      ? { status: "ready", bakery: outcome.data, error: null }
      : // A failed refresh keeps showing what was there.
        { status: previous.bakery ? "ready" : "error", bakery: previous.bakery, error: outcome.error },
  );
}

/** Runs a change after the previous one, adopts the state it returns, then re-lists. */
function change<T>(projectId: string, run: () => Promise<Outcome<T>>, stateOf: (data: T) => BakeryState) {
  const previous = queues.get(projectId) ?? Promise.resolve();
  const next = previous.then(async () => {
    const outcome = await run();
    if (outcome.ok) adopt(projectId, stateOf(outcome.data));
    void refetchBakery(projectId);
    return outcome;
  });
  queues.set(projectId, next.catch(() => {}));
  return next;
}

export function addFromPaths(projectId: string, paths: string[]): Promise<Outcome<BakeryAddResult>> {
  return change(projectId, () => call(() => commands.bakeryAddFromPaths(projectId, paths, null)), (data) => data.bakery);
}

export function addFromBytes(projectId: string, name: string, bytes: number[]): Promise<Outcome<BakeryAddResult>> {
  return change(projectId, () => call(() => commands.bakeryAddFromBytes(projectId, name, bytes)), (data) => data.bakery);
}

export function updateReference(projectId: string, id: string, patch: Partial<BakeryPatch>): Promise<Outcome<BakeryState>> {
  const full: BakeryPatch = { label: null, take: null, skip: null, tags: null, appliesTo: null, ...patch };
  return change(projectId, () => call(() => commands.bakeryUpdate(projectId, id, full)), (data) => data);
}

export function removeReference(projectId: string, id: string): Promise<Outcome<BakeryState>> {
  return change(projectId, () => call(() => commands.bakeryRemove(projectId, id)), (data) => data);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useBakery(projectId: string): BakeryData {
  const data = useSyncExternalStore(subscribe, () => entries.get(projectId) ?? LOADING);
  useEffect(() => {
    void refetchBakery(projectId);
  }, [projectId]);
  return data;
}
