import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type PromptFile } from "@/lib/bindings";
import { call } from "./call";

export type PromptsLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; files: PromptFile[] };

export interface StartChoices {
  /** Every prompt file in the project; filtered by agent where it's shown. */
  prompts: PromptsLoad;
  /** Reads the prompt files again (when a Start menu opens). */
  reload: () => void;
  /** Slice id → name, from `getPlan`. Null until loaded. */
  sliceNames: Record<string, string> | null;
  /** `getPlan` failing, as-is (slices then show their ids). */
  planError: AppError | null;
}

/**
 * What a Start menu offers: task prompts from `listPrompts` (re-read on every open, since the
 * Prompts page writes them) and slice names from `getPlan` (re-read on `plan:changed`).
 */
export function useStartChoices(projectId: string): StartChoices {
  const [prompts, setPrompts] = useState<PromptsLoad>({ status: "loading" });
  const [sliceNames, setSliceNames] = useState<Record<string, string> | null>(null);
  const [planError, setPlanError] = useState<AppError | null>(null);
  const latestPrompts = useRef(0);
  const latestPlan = useRef(0);
  const mounted = useRef(true);

  const reload = useCallback(() => {
    const request = ++latestPrompts.current;
    void call(() => commands.listPrompts(projectId)).then((outcome) => {
      if (!mounted.current || request !== latestPrompts.current) return;
      setPrompts(outcome.ok ? { status: "ready", files: outcome.data } : { status: "error", error: outcome.error });
    });
  }, [projectId]);

  const reloadPlan = useCallback(() => {
    const request = ++latestPlan.current;
    void call(() => commands.getPlan(projectId)).then((outcome) => {
      if (!mounted.current || request !== latestPlan.current) return;
      if (outcome.ok) {
        setSliceNames(Object.fromEntries(outcome.data.slices.map((slice) => [slice.id, slice.name])));
        setPlanError(null);
      } else setPlanError(outcome.error);
    });
  }, [projectId]);

  useEffect(() => {
    mounted.current = true;
    reload();
    reloadPlan();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.planChanged
      .listen(({ payload }) => {
        if (payload.projectId === projectId) reloadPlan();
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error) => console.warn("Couldn't listen to plan changes:", error));
    return () => {
      disposed = true;
      mounted.current = false;
      unlisten?.();
    };
  }, [projectId, reload, reloadPlan]);

  return { prompts, reload, sliceNames, planError };
}
