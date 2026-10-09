import { useEffect, useState } from "react";
import { commands, events, type AgentReport, type AppError, type Verification } from "@/lib/bindings";
import { call } from "./call";

export type Loaded<T> = { status: "loading" } | { status: "error"; error: AppError } | { status: "ready"; value: T };

/**
 * One slice's reports (`listReports`, every agent's) and last verification (`getVerification`),
 * read when the sheet opens, again whenever a verification ends (`finished`) and when one of the
 * slice's agents reports `report_ready` (a report changed, so the result may now be stale).
 *
 * `lastReporter` is the agent of the newest `report_ready` seen while the sheet is open: the
 * report written most recently. Reports carry no write time, so it's null until one arrives.
 */
export function useSliceDetail(projectId: string, sliceId: string, finished: number) {
  const [reports, setReports] = useState<Loaded<AgentReport[]>>({ status: "loading" });
  const [verification, setVerification] = useState<Loaded<Verification | null>>({ status: "loading" });
  const [reportChanged, setReportChanged] = useState(0);
  const [lastReporter, setLastReporter] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    void call(() => commands.listReports(projectId, sliceId)).then((outcome) => {
      if (!current) return;
      // A failed re-read keeps what's shown.
      if (outcome.ok) setReports({ status: "ready", value: outcome.data });
      else setReports((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
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
          setLastReporter(payload.agentId);
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

  return { reports, verification, lastReporter };
}
