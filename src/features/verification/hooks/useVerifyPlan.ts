import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type Plan } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type PlanLoad = { status: "loading" } | { status: "error"; error: AppError } | { status: "ready"; plan: Plan };

export interface VerifyPlan {
  load: PlanLoad;
  reload: () => void;
  /**
   * `setSliceStatus(done)` with the latest revision seen. `reason` makes it an override
   * (after `not_verified`), recorded by the backend as a decision.
   */
  markDone: (sliceId: string, reason?: string) => Promise<Outcome<Plan>>;
}

/**
 * The plan's slices (names, layers, statuses), fetched here rather than imported from another
 * feature, and again on `plan:changed`. Never adopts an older revision than it holds.
 */
export function useVerifyPlan(projectId: string): VerifyPlan {
  const [load, setLoad] = useState<PlanLoad>({ status: "loading" });
  const planRef = useRef<Plan | null>(null);
  const latestFetch = useRef(0);
  const mounted = useRef(true);

  const adopt = useCallback((plan: Plan) => {
    if (planRef.current && planRef.current.revision > plan.revision) return;
    planRef.current = plan;
    setLoad({ status: "ready", plan });
  }, []);

  const reload = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.getPlan(projectId)).then((outcome) => {
      if (!mounted.current || request !== latestFetch.current) return;
      if (outcome.ok) adopt(outcome.data);
      // A failed re-read keeps what's shown; only a failed first load replaces it.
      else if (!planRef.current) setLoad({ status: "error", error: outcome.error });
    });
  }, [projectId, adopt]);

  useEffect(() => {
    mounted.current = true;
    reload();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.planChanged
      .listen(({ payload }) => {
        if (payload.projectId === projectId && payload.revision !== planRef.current?.revision) reload();
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

  const markDone = useCallback(
    async (sliceId: string, reason?: string): Promise<Outcome<Plan>> => {
      const revision = planRef.current?.revision ?? null;
      const options = reason === undefined ? { override: false, reason: null } : { override: true, reason };
      const outcome = await call(() => commands.setSliceStatus(projectId, sliceId, "done", options, revision));
      if (outcome.ok && mounted.current) adopt(outcome.data);
      // The plan moved on: read it again (the refusal itself is shown as-is by the caller).
      else if (!outcome.ok && outcome.error.code === "stale_revision") reload();
      return outcome;
    },
    [projectId, adopt, reload],
  );

  return { load, reload, markDone };
}
