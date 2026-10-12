import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type SessionStatus } from "@/lib/bindings";
import { call, type Outcome } from "./call";

type Sessions = ReadonlyMap<string, { status: SessionStatus; since: number }>;

export type SessionsLoad = { status: "loading" } | { status: "error"; error: AppError } | { status: "ready"; sessions: Sessions };

const isLive = (status: SessionStatus) => status === "starting" || status === "running";

/**
 * The text a send types into a session: a bracketed paste (line breaks become carriage
 * returns, as xterm's own paste does), so a multi-line message arrives as one prompt instead of
 * one Enter per line. Enter goes separately, after the paste has ended.
 */
export function pasteSequence(message: string): string {
  return `\x1b[200~${message.replace(/\r?\n/g, "\r")}\x1b[201~`;
}

/**
 * Sending a prepared message to an agent's live session, the way the terminal types into it
 * (`agentWrite`). It never starts a session. Sessions come from `agentList`, merged with
 * `agent:status` by `since`, so "no live session" follows the agent as it starts or exits.
 */
export function useAgentSend(projectId: string) {
  const [load, setLoad] = useState<SessionsLoad>({ status: "loading" });
  const mounted = useRef(true);

  const merge = useCallback((list: { agentId: string; status: SessionStatus; since: number }[]) => {
    setLoad((previous) => {
      const next = new Map(previous.status === "ready" ? previous.sessions : []);
      for (const info of list) {
        const known = next.get(info.agentId);
        if (!known || info.since >= known.since) next.set(info.agentId, { status: info.status, since: info.since });
      }
      return { status: "ready", sessions: next };
    });
  }, []);

  const refresh = useCallback(async () => {
    const outcome = await call(() => commands.agentList(projectId));
    if (!mounted.current) return;
    if (outcome.ok) merge(outcome.data);
    else setLoad((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
  }, [projectId, merge]);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => {
      mounted.current = false;
    };
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.agentStatus
      .listen(({ payload }) => payload.projectId === projectId && merge([payload]))
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [projectId, merge]);

  const isAgentLive = useCallback(
    (agentId: string) => {
      const session = load.status === "ready" ? load.sessions.get(agentId) : undefined;
      return session !== undefined && isLive(session.status);
    },
    [load],
  );

  /** Pastes the message, then presses Enter. A refusal (e.g. the session just ended) comes back as-is. */
  const send = useCallback(
    async (agentId: string, message: string): Promise<Outcome<null>> => {
      const pasted = await call(() => commands.agentWrite(projectId, agentId, pasteSequence(message)));
      if (!pasted.ok) {
        void refresh();
        return pasted;
      }
      return call(() => commands.agentWrite(projectId, agentId, "\r"));
    },
    [projectId, refresh],
  );

  return { load, isAgentLive, refresh, send };
}
