import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events } from "@/lib/bindings";
import { call } from "./call";

/** How long a new session gets to report `session_started` through the hooks. */
export const HOOKS_GRACE_MS = 15_000;
/** Clock slack between this window and the hook helper's timestamps. */
const SLACK_MS = 5_000;

export interface HooksWatch {
  /** Agents whose current session hasn't reported `session_started` in time. */
  silent: ReadonlySet<string>;
  /** Starts waiting for `session_started` (after a successful start; `startedAt` is its `since`). */
  arm: (agentId: string, startedAt: number) => void;
}

/**
 * Notices hooks that aren't reporting. A hook that fails to run blocks nothing, and project
 * hooks don't run until the folder is trusted, so a session with no `session_started` event
 * ~15 s after it started is the only sign. Before warning, `agentEvents` is checked too, in
 * case the event arrived while nothing was listening. Cleared by the event, or when the
 * session ends (the warning is about that session).
 */
export function useHooksWatch(projectId: string): HooksWatch {
  const [silent, setSilent] = useState<ReadonlySet<string>>(() => new Set());
  const timers = useRef(new Map<string, number>());
  /** Per agent, when the watched session started; gone once it reported or ended. */
  const watching = useRef(new Map<string, number>());

  const clear = useCallback((agentId: string) => {
    window.clearTimeout(timers.current.get(agentId));
    timers.current.delete(agentId);
    watching.current.delete(agentId);
    setSilent((current) => {
      if (!current.has(agentId)) return current;
      const next = new Set(current);
      next.delete(agentId);
      return next;
    });
  }, []);

  const arm = useCallback(
    (agentId: string, startedAt: number) => {
      clear(agentId);
      watching.current.set(agentId, startedAt);
      const timer = window.setTimeout(() => {
        timers.current.delete(agentId);
        const query = { beforeId: null, limit: 50, minSeverity: null };
        void call(() => commands.agentEvents(projectId, agentId, query)).then((outcome) => {
          if (watching.current.get(agentId) !== startedAt) return;
          if (!outcome.ok) console.warn(`Couldn't read the events of ${agentId}:`, outcome.error.message);
          const reported =
            outcome.ok &&
            outcome.data.some((event) => event.kind === "session_started" && event.ts >= startedAt - SLACK_MS);
          if (reported) watching.current.delete(agentId);
          else setSilent((current) => new Set(current).add(agentId));
        });
      }, HOOKS_GRACE_MS);
      timers.current.set(agentId, timer);
    },
    [projectId, clear],
  );

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const keep = (stop: () => void) => {
      if (disposed) stop();
      else stops.push(stop);
    };
    events.agentEvent
      .listen(({ payload }) => {
        if (payload.projectId === projectId && payload.kind === "session_started") clear(payload.agentId);
      })
      .then(keep)
      .catch((error) => console.warn("Couldn't listen to agent events:", error));
    events.agentStatus
      .listen(({ payload }) => {
        if (payload.projectId === projectId && payload.status === "exited") clear(payload.agentId);
      })
      .then(keep)
      .catch((error) => console.warn("Couldn't listen to agent status:", error));
    const pending = timers.current;
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
      for (const timer of pending.values()) window.clearTimeout(timer);
      pending.clear();
    };
  }, [projectId, clear]);

  return { silent, arm };
}
