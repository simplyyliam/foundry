import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events } from "@/lib/bindings";
import type { UnreadWorst } from "../lib/events";
import { call } from "./call";
import { unreadWorstOf } from "./unread";

/** Bursts of events (an agent editing many files) refresh once. */
const DEBOUNCE_MS = 250;

export interface Unread {
  /** Per agent id, the worst unread severity; missing or null when there's nothing unread. */
  worst: Record<string, UnreadWorst>;
  /** The backend's message when the counts couldn't be read. */
  error: string | null;
  /** Reads again (after marking read). */
  refresh: () => void;
}

/**
 * Unread state for the list's dots: `agentEventCounts` on mount, again on every
 * `agent:event` for this project, and when the detail marks something read.
 */
export function useUnread(projectId: string): Unread {
  const [worst, setWorst] = useState<Record<string, UnreadWorst>>({});
  const [error, setError] = useState<string | null>(null);
  const latestFetch = useRef(0);

  const refresh = useCallback(() => {
    const request = ++latestFetch.current;
    void (async () => {
      const counts = await call(() => commands.agentEventCounts(projectId));
      if (request !== latestFetch.current) return;
      if (!counts.ok) {
        setError(counts.error.message);
        return;
      }
      const results = await Promise.all(counts.data.map((entry) => unreadWorstOf(projectId, entry)));
      if (request !== latestFetch.current) return;
      const next: Record<string, UnreadWorst> = {};
      let failure: string | null = null;
      results.forEach((result, index) => {
        if (result.ok) next[counts.data[index].agentId] = result.data;
        else failure ??= result.error.message;
      });
      setWorst(next);
      setError(failure);
    })();
  }, [projectId]);

  useEffect(() => {
    refresh();
    let timer: number | undefined;
    let disposed = false;
    let stop: (() => void) | undefined;
    events.agentEvent
      .listen(({ payload }) => {
        if (payload.projectId !== projectId) return;
        window.clearTimeout(timer);
        timer = window.setTimeout(refresh, DEBOUNCE_MS);
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch((reason) => setError(reason instanceof Error ? reason.message : String(reason)));
    return () => {
      disposed = true;
      stop?.();
      window.clearTimeout(timer);
      latestFetch.current++;
    };
  }, [projectId, refresh]);

  return { worst, error, refresh };
}
