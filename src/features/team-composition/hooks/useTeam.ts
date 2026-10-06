import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  type AppError,
  type AvatarChoice,
  type RosterAgent,
  type TeamAgent,
  type TeamProposal,
  type TeamWritten,
} from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type TeamLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  /** `agents` is null while the project has no confirmed team. */
  | { status: "ready"; agents: TeamAgent[] | null };

export interface TeamController {
  load: TeamLoad;
  reload: () => Promise<void>;
  /** Asks Claude for a proposal (uses the subscription). Writes nothing. */
  compose: () => Promise<Outcome<TeamProposal>>;
  /** Writes the roster, then reads the team again. */
  confirm: (roster: RosterAgent[]) => Promise<Outcome<TeamWritten>>;
  /** Adds one agent to the confirmed team; adopts the team it returns. */
  addAgent: (agent: RosterAgent) => Promise<Outcome<TeamWritten>>;
  /** Adopts the agent it returns. */
  setAvatar: (agentId: string, choice: AvatarChoice) => Promise<Outcome<TeamAgent>>;
}

/**
 * The project's confirmed team (`getTeam`), and the commands that change it. Changes run one
 * at a time; `compose` doesn't queue (it runs for minutes and changes nothing). A
 * `team_confirmed` answer means the team exists after all, so the team is read again.
 */
export function useTeam(projectId: string): TeamController {
  const [load, setLoad] = useState<TeamLoad>({ status: "loading" });
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const latestFetch = useRef(0);
  const mounted = useRef(true);

  const enqueue = useCallback(<T,>(task: () => Promise<T>): Promise<T> => {
    const run = queue.current.then(task);
    queue.current = run.catch(() => undefined);
    return run;
  }, []);

  const fetchTeam = useCallback(async () => {
    const request = ++latestFetch.current;
    const outcome = await call(() => commands.getTeam(projectId));
    if (!mounted.current || request !== latestFetch.current) return;
    if (outcome.ok) setLoad({ status: "ready", agents: outcome.data?.agents ?? null });
    // A failed re-read keeps what's shown; only a failed first load replaces it.
    else setLoad((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
  }, [projectId]);

  useEffect(() => {
    mounted.current = true;
    void fetchTeam();
    return () => {
      mounted.current = false;
    };
  }, [fetchTeam]);

  const reload = useCallback(async () => {
    setLoad((previous) => (previous.status === "error" ? { status: "loading" } : previous));
    await fetchTeam();
  }, [fetchTeam]);

  /** Reads the team again when the backend says it's already confirmed. */
  const watch = useCallback(
    <T,>(outcome: Outcome<T>): Outcome<T> => {
      if (!outcome.ok && outcome.error.code === "team_confirmed") void fetchTeam();
      return outcome;
    },
    [fetchTeam],
  );

  const compose = useCallback(
    async () => watch(await call(() => commands.composeTeam(projectId))),
    [projectId, watch],
  );

  const confirm = useCallback(
    (roster: RosterAgent[]) =>
      enqueue(async () => {
        const outcome = watch(await call(() => commands.confirmTeam(projectId, roster)));
        if (outcome.ok) await fetchTeam();
        return outcome;
      }),
    [projectId, enqueue, watch, fetchTeam],
  );

  const addAgent = useCallback(
    (agent: RosterAgent) =>
      enqueue(async () => {
        const outcome = await call(() => commands.addAgentManually(projectId, agent));
        if (outcome.ok && mounted.current) {
          latestFetch.current++; // An older read in flight mustn't drop the new agent.
          setLoad({ status: "ready", agents: outcome.data.team.agents });
        }
        return outcome;
      }),
    [projectId, enqueue],
  );

  const setAvatar = useCallback(
    (agentId: string, choice: AvatarChoice) =>
      enqueue(async () => {
        const outcome = await call(() => commands.setAgentAvatar(projectId, agentId, choice));
        if (outcome.ok && mounted.current) {
          const updated = outcome.data;
          setLoad((previous) =>
            previous.status === "ready" && previous.agents
              ? { ...previous, agents: previous.agents.map((agent) => (agent.id === updated.id ? updated : agent)) }
              : previous,
          );
        }
        return outcome;
      }),
    [projectId, enqueue],
  );

  return { load, reload, compose, confirm, addAgent, setAvatar };
}
