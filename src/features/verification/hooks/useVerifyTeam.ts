import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type TeamAgent } from "@/lib/bindings";
import { useTeamChanged } from "@/shared";
import { call } from "./call";

/**
 * The confirmed team (`getTeam`), only to name the agents behind reports and findings. Read on
 * mount and on the frontend's team-changed signal (there's no backend event). Until it loads,
 * or if it fails or there's no team, agents show by id: the error is logged, not shown, since
 * nothing on this page depends on it.
 */
export function useVerifyTeam(projectId: string): ReadonlyMap<string, TeamAgent> {
  const [agents, setAgents] = useState<ReadonlyMap<string, TeamAgent>>(() => new Map());
  const latestFetch = useRef(0);

  const reload = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.getTeam(projectId)).then((outcome) => {
      if (request !== latestFetch.current) return;
      if (outcome.ok) setAgents(new Map((outcome.data?.agents ?? []).map((agent) => [agent.id, agent])));
      else console.error("getTeam failed", outcome.error);
    });
  }, [projectId]);

  useEffect(() => {
    reload();
    const fetches = latestFetch;
    return () => {
      fetches.current++;
    };
  }, [reload]);

  useTeamChanged(projectId, reload);
  return agents;
}
