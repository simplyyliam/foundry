import { useEffect, useSyncExternalStore } from "react";
import { commands, type AppError, type RoadmapResult } from "@/lib/bindings";
import { call } from "./call";

/**
 * Per-project cache of `getRoadmap` plus whether the brain dump is empty, shared by every
 * Roadmap view (the pane and the pop-out show the same data). Refetched whenever a view
 * mounts, and after a generation finishes.
 */

export interface RoadmapData {
  /** "loading" only until the first response for this project. */
  status: "loading" | "ready" | "error";
  /** The saved roadmap, or null when none has been generated yet. */
  result: RoadmapResult | null;
  /** `docs/brain-dump.md` has no text yet. Null until known. */
  briefEmpty: boolean | null;
  error: AppError | null;
}

const LOADING: RoadmapData = { status: "loading", result: null, briefEmpty: null, error: null };

const entries = new Map<string, RoadmapData>();
const latestRequest = new Map<string, number>();
const listeners = new Set<() => void>();

function publish(projectId: string, data: RoadmapData) {
  entries.set(projectId, data);
  listeners.forEach((listener) => listener());
}

/** Re-reads the saved roadmap and the brain dump. Out-of-order responses are dropped. */
export async function refetchRoadmap(projectId: string): Promise<void> {
  const request = (latestRequest.get(projectId) ?? 0) + 1;
  latestRequest.set(projectId, request);
  const [roadmap, brief] = await Promise.all([
    call(() => commands.getRoadmap(projectId)),
    call(() => commands.getDocument(projectId)),
  ]);
  if (latestRequest.get(projectId) !== request) return;

  const previous = entries.get(projectId) ?? LOADING;
  publish(projectId, {
    status: roadmap.ok ? "ready" : "error",
    // A failed refresh keeps showing what was there.
    result: roadmap.ok ? roadmap.data : previous.result,
    briefEmpty: brief.ok ? brief.data.trim() === "" : previous.briefEmpty,
    error: roadmap.ok ? null : roadmap.error,
  });
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useRoadmap(projectId: string): RoadmapData {
  const data = useSyncExternalStore(subscribe, () => entries.get(projectId) ?? LOADING);
  useEffect(() => {
    void refetchRoadmap(projectId);
  }, [projectId]);
  return data;
}
