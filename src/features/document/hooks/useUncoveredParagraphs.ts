import { useEffect, useState } from "react";
import { commands, events, type AppError } from "@/lib/bindings";
import { call } from "./call";

export interface UncoveredParagraphs {
  /** Brain-dump paragraphs the saved roadmap didn't pick up; empty without a roadmap. */
  paragraphs: readonly string[];
  error: AppError | null;
}

const NONE: readonly string[] = [];

/**
 * The document's own read of `getRoadmap` (it doesn't share the roadmap feature's store),
 * refetched when a generation for this project reaches `done`.
 */
export function useUncoveredParagraphs(projectId: string): UncoveredParagraphs {
  const [state, setState] = useState<UncoveredParagraphs & { projectId: string | null }>({
    projectId: null,
    paragraphs: NONE,
    error: null,
  });

  useEffect(() => {
    let latest = 0;
    let disposed = false;
    const refetch = async () => {
      const request = ++latest;
      const outcome = await call(() => commands.getRoadmap(projectId));
      if (disposed || request !== latest) return;
      setState((previous) => ({
        projectId,
        // A failed refresh keeps the last highlights.
        paragraphs: outcome.ok ? (outcome.data?.uncoveredParagraphs ?? NONE) : previous.projectId === projectId ? previous.paragraphs : NONE,
        error: outcome.ok ? null : outcome.error,
      }));
    };

    void refetch();
    let unlisten: (() => void) | undefined;
    events.roadmapProgress
      .listen(({ payload }) => {
        if (payload.projectId === projectId && payload.stage === "done") void refetch();
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

  return state.projectId === projectId ? state : { paragraphs: NONE, error: null };
}
