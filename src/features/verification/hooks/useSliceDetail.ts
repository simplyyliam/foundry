import { useEffect, useState } from "react";
import { commands, events, type AppError, type SliceReport, type Verification } from "@/lib/bindings";
import { call } from "./call";

export type Loaded<T> = { status: "loading" } | { status: "error"; error: AppError } | { status: "ready"; value: T };

/**
 * One slice's report (`getReport`) and last verification (`getVerification`), read when the
 * sheet opens, again whenever a verification ends (`finished`) and when the slice's agent
 * reports `report_ready` (the report changed, so the result may now be stale).
 */
export function useSliceDetail(projectId: string, sliceId: string, finished: number) {
  const [report, setReport] = useState<Loaded<SliceReport | null>>({ status: "loading" });
  const [verification, setVerification] = useState<Loaded<Verification | null>>({ status: "loading" });
  const [reportChanged, setReportChanged] = useState(0);

  useEffect(() => {
    let current = true;
    void call(() => commands.getReport(projectId, sliceId)).then((outcome) => {
      if (!current) return;
      // A failed re-read keeps what's shown.
      if (outcome.ok) setReport({ status: "ready", value: outcome.data });
      else setReport((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
    void call(() => commands.getVerification(projectId, sliceId)).then((outcome) => {
      if (!current) return;
      if (outcome.ok) setVerification({ status: "ready", value: outcome.data });
      else setVerification((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
    return () => {
      current = false;
    };
  }, [projectId, sliceId, finished, reportChanged]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.agentEvent
      .listen(({ payload }) => {
        if (payload.projectId === projectId && payload.sliceId === sliceId && payload.kind === "report_ready") {
          setReportChanged((count) => count + 1);
        }
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
  }, [projectId, sliceId]);

  return { report, verification };
}
