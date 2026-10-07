import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type TeamAgent } from "@/lib/bindings";
import { useTeamChanged } from "@/shared";
import { call } from "./call";

export type TeamLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  /** `agents` is null while the project has no confirmed team. */
  | { status: "ready"; agents: TeamAgent[] | null };

/** The confirmed team. Read on mount and when an agent is added in this window. */
export function useTeam(projectId: string): TeamLoad {
  const [load, setLoad] = useState<TeamLoad>({ status: "loading" });
  const latestFetch = useRef(0);

  const fetchTeam = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.getTeam(projectId)).then((outcome) => {
      if (request !== latestFetch.current) return;
      if (outcome.ok) setLoad({ status: "ready", agents: outcome.data?.agents ?? null });
      // A failed re-read keeps the list; only a failed first load replaces it.
      else setLoad((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
  }, [projectId]);

  useEffect(() => {
    fetchTeam();
    return () => {
      latestFetch.current++;
    };
  }, [fetchTeam]);

  useTeamChanged(projectId, fetchTeam);

  return load;
}
