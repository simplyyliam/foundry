import { useCallback, useEffect, useState } from "react";
import { commands } from "@/lib/bindings";
import { DEFAULT_AVATAR } from "../lib/avatar";
import { call } from "./call";

/**
 * Each agent's avatar as a data URL from `agentAvatar`, fetched once per agent. Null or an
 * error shows the default avatar (errors are logged, not shown: the default is the answer).
 */
export function useAgentAvatars(projectId: string, agentIds: readonly string[]): (agentId: string) => string {
  const [urls, setUrls] = useState<Record<string, string | null>>({});

  const key = agentIds.join("\n");
  useEffect(() => {
    let cancelled = false;
    for (const agentId of key ? key.split("\n") : []) {
      void call(() => commands.agentAvatar(projectId, agentId)).then((outcome) => {
        if (cancelled) return;
        if (!outcome.ok) console.warn(`Couldn't load the avatar of ${agentId}:`, outcome.error.message);
        setUrls((current) => ({ ...current, [agentId]: outcome.ok ? outcome.data : null }));
      });
    }
    return () => {
      cancelled = true;
    };
  }, [projectId, key]);

  return useCallback((agentId: string) => urls[agentId] ?? DEFAULT_AVATAR, [urls]);
}
