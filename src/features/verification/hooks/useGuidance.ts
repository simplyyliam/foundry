import { useEffect, useState } from "react";
import { commands, events, type VerificationGuidance } from "@/lib/bindings";
import { call } from "./call";
import type { Loaded } from "./useSliceDetail";

/**
 * What to do about one slice (`verificationGuidance`): read when the sheet opens, whenever a
 * verification ends (`finished`), on `plan:changed` for the project (status, slices, agents),
 * on `report_ready` for the slice (a report can end "waiting" or make the result stale), and
 * when the slice's skip is recorded or cleared. Only the newest read is kept.
 */
export function useGuidance(projectId: string, sliceId: string, finished: number) {
  const [guidance, setGuidance] = useState<Loaded<VerificationGuidance>>({ status: "loading" });
  const [changed, setChanged] = useState(0);

  useEffect(() => {
    let current = true;
    void call(() => commands.verificationGuidance(projectId, sliceId)).then((outcome) => {
      // Unmounted or superseded by a newer read: ignore the answer.
      if (!current) return;
      // A failed re-read keeps what's shown.
      if (outcome.ok) setGuidance({ status: "ready", value: outcome.data });
      else setGuidance((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
    return () => {
      current = false;
    };
  }, [projectId, sliceId, finished, changed]);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const keep = (stop: () => void) => (disposed ? stop() : stops.push(stop));
    const bump = () => setChanged((count) => count + 1);
    events.planChanged
      .listen(({ payload }) => payload.projectId === projectId && bump())
      .then(keep)
      .catch(() => {});
    events.agentEvent
      .listen(({ payload }) => {
        if (payload.projectId === projectId && payload.sliceId === sliceId && payload.kind === "report_ready") bump();
      })
      .then(keep)
      .catch(() => {});
    events.verifySkipped
      .listen(({ payload }) => payload.projectId === projectId && payload.skip.sliceId === sliceId && bump())
      .then(keep)
      .catch(() => {});
    events.verifySkipCleared
      .listen(({ payload }) => payload.projectId === projectId && payload.sliceId === sliceId && bump())
      .then(keep)
      .catch(() => {});
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, [projectId, sliceId]);

  return guidance;
}
