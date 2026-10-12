import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type SessionStatus } from "@/lib/bindings";
import type { CommitStatusLoad, SliceCommitController } from "@/shared";
import { call } from "./call";

type Sessions = ReadonlyMap<string, { status: SessionStatus; since: number }>;

const isLive = (status: SessionStatus) => status === "starting" || status === "running";

/**
 * Committing one slice: `commitStatus` for it (files, the type and message `commitSlice` would
 * use) and whether any agent session in the project is live (`agentList` merged with
 * `agent:status` by `since`). Read on mount, on `agent:status` and `plan:changed` for the
 * project, on window focus (an edit made outside), and after a commit.
 */
export function useSliceCommit(projectId: string, sliceId: string): SliceCommitController {
  const [status, setStatus] = useState<CommitStatusLoad>({ status: "loading" });
  const [sessions, setSessions] = useState<Sessions>(new Map());
  const latestFetch = useRef(0);
  const mounted = useRef(true);

  const merge = useCallback((list: { agentId: string; status: SessionStatus; since: number }[]) => {
    setSessions((previous) => {
      const next = new Map(previous);
      for (const info of list) {
        const known = next.get(info.agentId);
        if (!known || info.since >= known.since) next.set(info.agentId, { status: info.status, since: info.since });
      }
      return next;
    });
  }, []);

  const refresh = useCallback(async () => {
    const request = ++latestFetch.current;
    const [commit, list] = await Promise.all([
      call(() => commands.commitStatus(projectId, sliceId)),
      call(() => commands.agentList(projectId)),
    ]);
    if (!mounted.current) return;
    // Unknown sessions don't block: `commitSlice` refuses a live one itself (shown as-is).
    if (list.ok) merge(list.data);
    else console.error("agentList failed", list.error);
    if (request !== latestFetch.current) return;
    setStatus(commit.ok ? { status: "ready", value: commit.data } : { status: "error", error: commit.error });
  }, [projectId, sliceId, merge]);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => {
      mounted.current = false;
    };
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const keep = (stop: () => void) => (disposed ? stop() : stops.push(stop));
    events.agentStatus
      .listen(({ payload }) => {
        if (payload.projectId !== projectId) return;
        merge([payload]);
        void refresh();
      })
      .then(keep)
      .catch(() => {});
    events.planChanged
      .listen(({ payload }) => payload.projectId === projectId && void refresh())
      .then(keep)
      .catch(() => {});
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
      window.removeEventListener("focus", onFocus);
    };
  }, [projectId, refresh, merge]);

  const commit = useCallback(
    async (message: string | null) => {
      const outcome = await call(() => commands.commitSlice(projectId, sliceId, message));
      if (mounted.current) void refresh();
      return outcome;
    },
    [projectId, sliceId, refresh],
  );

  const sessionLive = [...sessions.values()].some((session) => isLive(session.status));
  return { status, sessionLive, refresh, commit };
}
