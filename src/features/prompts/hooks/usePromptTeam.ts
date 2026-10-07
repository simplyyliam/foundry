import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type TeamAgent } from "@/lib/bindings";
import { call } from "./call";

export type TeamLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  /** `agents` is null while the project has no confirmed team. */
  | { status: "ready"; agents: TeamAgent[] | null };

/**
 * The confirmed team (`getTeam`), read-only, for the agent picker and the prompt list. There's
 * no team-changed event; the team is confirmed on the Setup page, so leaving for it and
 * coming back mounts this again.
 */
export function usePromptTeam(projectId: string): { load: TeamLoad; reload: () => void } {
  const [load, setLoad] = useState<TeamLoad>({ status: "loading" });
  const latestFetch = useRef(0);

  const fetchTeam = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.getTeam(projectId)).then((outcome) => {
      if (request !== latestFetch.current) return;
      if (outcome.ok) setLoad({ status: "ready", agents: outcome.data?.agents ?? null });
      else setLoad((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
  }, [projectId]);

  useEffect(() => {
    fetchTeam();
    const fetches = latestFetch;
    return () => {
      fetches.current++;
    };
  }, [fetchTeam]);

  const reload = useCallback(() => {
    setLoad((previous) => (previous.status === "error" ? { status: "loading" } : previous));
    fetchTeam();
  }, [fetchTeam]);

  return { load, reload };
}
