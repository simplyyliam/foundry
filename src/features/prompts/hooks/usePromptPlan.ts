import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type Plan } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type PlanLoad = { status: "loading" } | { status: "error"; error: AppError } | { status: "ready"; plan: Plan };

export interface PromptPlan {
  load: PlanLoad;
  reload: () => void;
  /** Reads the plan again, then adds a decision to the slice with that fresh revision. */
  addDecision: (sliceId: string, text: string) => Promise<Outcome<Plan>>;
  /** A fresh `getPlan`, for reading what's true right now (not what's shown). */
  readPlan: () => Promise<Outcome<Plan>>;
}

/**
 * The plan's slices (names, layers, statuses) for the slice picker and the prompt list.
 * Fetched here rather than imported from another feature, and again on `plan:changed`.
 */
export function usePromptPlan(projectId: string): PromptPlan {
  const [load, setLoad] = useState<PlanLoad>({ status: "loading" });
  const latestFetch = useRef(0);
  const mounted = useRef(true);

  const adopt = useCallback((plan: Plan) => {
    setLoad((previous) =>
      previous.status === "ready" && previous.plan.revision > plan.revision ? previous : { status: "ready", plan },
    );
  }, []);

  const readPlan = useCallback(() => call(() => commands.getPlan(projectId)), [projectId]);

  const reload = useCallback(() => {
    const request = ++latestFetch.current;
    void readPlan().then((outcome) => {
      if (!mounted.current || request !== latestFetch.current) return;
      if (outcome.ok) adopt(outcome.data);
      // A failed re-read keeps what's shown; only a failed first load replaces it.
      else setLoad((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
  }, [readPlan, adopt]);

  useEffect(() => {
    mounted.current = true;
    reload();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.planChanged
      .listen(({ payload }) => {
        if (payload.projectId === projectId) reload();
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    const fetches = latestFetch;
    return () => {
      disposed = true;
      mounted.current = false;
      fetches.current++;
      unlisten?.();
    };
  }, [projectId, reload]);

  const addDecision = useCallback(
    async (sliceId: string, text: string): Promise<Outcome<Plan>> => {
      const fresh = await readPlan();
      if (!fresh.ok) return fresh;
      const outcome = await call(() => commands.addDecision(projectId, sliceId, text, fresh.data.revision));
      if (outcome.ok && mounted.current) adopt(outcome.data);
      return outcome;
    },
    [projectId, readPlan, adopt],
  );

  return { load, reload, addDecision, readPlan };
}
