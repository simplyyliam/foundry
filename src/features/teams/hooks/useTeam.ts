import { useEffect, useState } from "react";
import { commands, type AppError, type TeamAgent } from "@/lib/bindings";
import { call } from "./call";

export type TeamLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; agents: TeamAgent[] | null };

/**
 * The confirmed team (null when there's none). Read once on mount: there's no team-changed
 * event, and confirming happens on Setup, so coming back here remounts the page.
 */
export function useTeam(projectId: string): TeamLoad {
  const [load, setLoad] = useState<TeamLoad>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    void call(() => commands.getTeam(projectId)).then((outcome) => {
      if (cancelled) return;
      setLoad(outcome.ok ? { status: "ready", agents: outcome.data?.agents ?? null } : { status: "error", error: outcome.error });
    });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  return load;
}
