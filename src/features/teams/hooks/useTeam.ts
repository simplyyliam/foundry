import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type TeamAgent } from "@/lib/bindings";
import { useTeamChanged } from "@/shared";
import { call } from "./call";

export type TeamLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; agents: TeamAgent[] | null };

/**
 * The confirmed team (null when there's none). Read on mount, and again when an agent is
 * added from this window (`useTeamChanged`; there's no backend team event).
 */
export function useTeam(projectId: string): TeamLoad {
  const [load, setLoad] = useState<TeamLoad>({ status: "loading" });
  const latestFetch = useRef(0);

  const fetchTeam = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.getTeam(projectId)).then((outcome) => {
      if (request !== latestFetch.current) return;
      if (outcome.ok) setLoad({ status: "ready", agents: outcome.data?.agents ?? null });
      // A failed re-read keeps the cards; only a failed first load replaces them.
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
