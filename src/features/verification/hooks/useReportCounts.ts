import { useEffect, useState } from "react";
import { commands, events } from "@/lib/bindings";
import { call } from "./call";

/**
 * How many reports each slice has, for the row's "Report" chip. The digest only says whether
 * there's at least one, so `listReports` is read for every slice that has one: on mount, when
 * a slice gets its first report, and on `report_ready` for that slice. A failed read leaves
 * that slice without a count (the chip then shows no number).
 */
export function useReportCounts(projectId: string, withReports: readonly string[]): Readonly<Record<string, number>> {
  const [counts, setCounts] = useState<Readonly<Record<string, number>>>({});
  const key = withReports.join("\n");

  useEffect(() => {
    let current = true;
    const latest = new Map<string, number>();
    const read = (sliceId: string) => {
      const request = (latest.get(sliceId) ?? 0) + 1;
      latest.set(sliceId, request);
      void call(() => commands.listReports(projectId, sliceId)).then((outcome) => {
        if (!current || latest.get(sliceId) !== request) return;
        setCounts((previous) => {
          const { [sliceId]: _old, ...rest } = previous;
          return outcome.ok ? { ...rest, [sliceId]: outcome.data.length } : rest;
        });
      });
    };
    const sliceIds = key === "" ? [] : key.split("\n");
    sliceIds.forEach(read);

    let unlisten: (() => void) | undefined;
    events.agentEvent
      .listen(({ payload }) => {
        if (payload.projectId === projectId && payload.kind === "report_ready" && payload.sliceId) read(payload.sliceId);
      })
      .then((stop) => {
        if (current) unlisten = stop;
        else stop();
      })
      .catch(() => {});
    return () => {
      current = false;
      unlisten?.();
    };
  }, [projectId, key]);

  return counts;
}
