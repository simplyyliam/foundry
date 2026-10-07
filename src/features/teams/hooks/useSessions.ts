import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AgentStartOptions, type AppError, type SessionInfo } from "@/lib/bindings";
import type { SessionView } from "../lib/session";
import { call, type Outcome } from "./call";

export interface Sessions {
  /** By agent id; agents with no session have no entry. */
  byAgent: Record<string, SessionView>;
  /** `agentList` failing, as-is. */
  error: AppError | null;
  reload: () => void;
  start: (agentId: string, options: AgentStartOptions) => Promise<Outcome<SessionInfo>>;
  /** `not_found` (already ended) refetches the list and counts as done. */
  stop: (agentId: string, force: boolean) => Promise<Outcome<null>>;
}

const toView = (info: { status: SessionView["status"]; exitCode: number | null; since: number }): SessionView => ({
  status: info.status,
  exitCode: info.exitCode,
  since: info.since,
});

/** Keeps whichever report is newer (`since`), so a late answer never undoes a status event. */
function newer(current: SessionView | undefined, next: SessionView): SessionView {
  return current && current.since > next.since ? current : next;
}

/** The project's sessions from `agentList`, kept current by `agent:status`. */
export function useSessions(projectId: string): Sessions {
  const [byAgent, setByAgent] = useState<Record<string, SessionView>>({});
  const [error, setError] = useState<AppError | null>(null);
  const latest = useRef(0);
  const mounted = useRef(true);

  const reload = useCallback(() => {
    const request = ++latest.current;
    const askedAt = Date.now();
    void call(() => commands.agentList(projectId)).then((outcome) => {
      if (!mounted.current || request !== latest.current) return;
      if (!outcome.ok) {
        setError(outcome.error);
        return;
      }
      setError(null);
      setByAgent((current) => {
        const next: Record<string, SessionView> = {};
        for (const info of outcome.data) next[info.agentId] = newer(current[info.agentId], toView(info));
        // Not listed: keep only what an event reported after we asked.
        for (const [agentId, view] of Object.entries(current)) {
          if (!(agentId in next) && view.since > askedAt) next[agentId] = view;
        }
        return next;
      });
    });
  }, [projectId]);

  useEffect(() => {
    mounted.current = true;
    reload();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.agentStatus
      .listen(({ payload }) => {
        if (payload.projectId !== projectId) return;
        setByAgent((current) => ({ ...current, [payload.agentId]: newer(current[payload.agentId], toView(payload)) }));
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error) => console.warn("Couldn't listen to agent status:", error));
    return () => {
      disposed = true;
      mounted.current = false;
      unlisten?.();
    };
  }, [projectId, reload]);

  const start = useCallback(
    async (agentId: string, options: AgentStartOptions) => {
      const outcome = await call(() => commands.agentStart(projectId, agentId, options));
      if (outcome.ok && mounted.current) {
        setByAgent((current) => ({ ...current, [agentId]: newer(current[agentId], toView(outcome.data)) }));
      }
      return outcome;
    },
    [projectId],
  );

  const stop = useCallback(
    async (agentId: string, force: boolean): Promise<Outcome<null>> => {
      const outcome = await call(() => commands.agentStop(projectId, agentId, { force }));
      if (!outcome.ok && outcome.error.code === "not_found") {
        reload();
        return { ok: true, data: null };
      }
      return outcome;
    },
    [projectId, reload],
  );

  return { byAgent, error, reload, start, stop };
}
