import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type PlanSummary, type SliceView } from "@/lib/bindings";
import { call } from "./call";

export type PlanStatus =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; summary: PlanSummary; slices: SliceView[] };

/**
 * What Propose needs from the plan: `planSummary` (is it confirmed?) and `getPlan` (are there
 * slices, and their names for the cards). Fetched here rather than imported from another
 * feature, and fetched again on `plan:changed` for this project.
 */
export function usePlanStatus(projectId: string): PlanStatus {
  const [status, setStatus] = useState<PlanStatus>({ status: "loading" });
  const latestFetch = useRef(0);

  const refetch = useCallback(async () => {
    const request = ++latestFetch.current;
    const [summary, plan] = await Promise.all([
      call(() => commands.planSummary(projectId)),
      call(() => commands.getPlan(projectId)),
    ]);
    if (request !== latestFetch.current) return;
    if (!summary.ok) setStatus({ status: "error", error: summary.error });
    else if (!plan.ok) setStatus({ status: "error", error: plan.error });
    else setStatus({ status: "ready", summary: summary.data, slices: plan.data.slices });
  }, [projectId]);

  useEffect(() => {
    void refetch();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.planChanged
      .listen(({ payload }) => {
        if (payload.projectId === projectId) void refetch();
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      latestFetch.current++;
      unlisten?.();
    };
  }, [projectId, refetch]);

  return status;
}
