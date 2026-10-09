import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type ManagerDigest, type VerifySkip } from "@/lib/bindings";
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
 *
 * Each slice's `skip` is the backend's skip store, the only record of it: `verify:skipped` and
 * `verify:skip_cleared` patch that slice's `skip` at once, then the digest is read again. A read
 * already in flight is dropped, so its older answer can't bring a cleared skip back.
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

  const setSkip = useCallback(
    (sliceId: string, skip: VerifySkip | null) => {
      latestFetch.current++;
      setLoad((previous) =>
        previous.status === "ready"
          ? {
              status: "ready",
              digest: {
                ...previous.digest,
                slices: previous.digest.slices.map((slice) => (slice.sliceId === sliceId ? { ...slice, skip } : slice)),
              },
            }
          : previous,
      );
      reload();
    },
    [reload],
  );

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
    events.verifySkipped
      .listen(({ payload }) => payload.projectId === projectId && setSkip(payload.skip.sliceId, payload.skip))
      .then(keep)
      .catch(() => {});
    events.verifySkipCleared
      .listen(({ payload }) => payload.projectId === projectId && setSkip(payload.sliceId, null))
      .then(keep)
      .catch(() => {});
    const fetches = latestFetch;
    return () => {
      disposed = true;
      clearTimeout(timer);
      fetches.current++;
      stops.forEach((stop) => stop());
    };
  }, [projectId, reload, setSkip]);

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
