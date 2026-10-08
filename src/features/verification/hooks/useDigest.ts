import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type ManagerDigest } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type DigestLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; digest: ManagerDigest };

/** Agent events can come in bursts; one re-read per quiet moment is enough. */
const EVENT_DEBOUNCE_MS = 400;

/**
 * `managerDigest`: per slice whether a report exists and its verification state, plus the
 * attention items. Read again on `plan:changed`, on any `agent:event` for the project
 * (reports and unread warnings live there) and whenever a verification ends (`finished`).
 */
export function useDigest(projectId: string, finished: number) {
  const [load, setLoad] = useState<DigestLoad>({ status: "loading" });
  const hasDigest = useRef(false);
  const latestFetch = useRef(0);

  const adopt = useCallback((digest: ManagerDigest) => {
    hasDigest.current = true;
    setLoad({ status: "ready", digest });
  }, []);

  const reload = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.managerDigest(projectId)).then((outcome) => {
      if (request !== latestFetch.current) return;
      if (outcome.ok) adopt(outcome.data);
      // A failed re-read keeps what's shown; only a failed first load replaces it.
      else if (!hasDigest.current) setLoad({ status: "error", error: outcome.error });
    });
  }, [projectId, adopt]);

  useEffect(() => {
    reload();
  }, [reload, finished]);

  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const stops: (() => void)[] = [];
    const keep = (stop: () => void) => (disposed ? stop() : stops.push(stop));
    const later = () => {
      clearTimeout(timer);
      timer = setTimeout(reload, EVENT_DEBOUNCE_MS);
    };
    events.planChanged
      .listen(({ payload }) => payload.projectId === projectId && reload())
      .then(keep)
      .catch(() => {});
    events.agentEvent
      .listen(({ payload }) => payload.projectId === projectId && later())
      .then(keep)
      .catch(() => {});
    const fetches = latestFetch;
    return () => {
      disposed = true;
      clearTimeout(timer);
      fetches.current++;
      stops.forEach((stop) => stop());
    };
  }, [projectId, reload]);

  /** Writes `.foundry/reviews/digest.md` and shows the digest it returns. */
  const writeFile = useCallback(async (): Promise<Outcome<ManagerDigest>> => {
    const outcome = await call(() => commands.writeDigestFile(projectId));
    if (outcome.ok) {
      latestFetch.current++;
      adopt(outcome.data);
    }
    return outcome;
  }, [projectId, adopt]);

  return { load, reload, writeFile };
}
