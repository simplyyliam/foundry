import { useCallback, useEffect, useRef, useState } from "react";
import { commands } from "@/lib/bindings";
import { DEFAULT_AVATAR } from "../lib/avatar";
import { call } from "./call";

export interface AgentAvatars {
  /** The agent's image, or the default avatar (no avatar, not loaded yet, or an error). */
  srcOf: (agentId: string) => string;
  /** Reads one agent's avatar again (after it changed). */
  refresh: (agentId: string) => void;
}

/**
 * Each agent's avatar as a data URL from `agentAvatar`, fetched once per agent and kept.
 * Null or any error shows the default avatar.
 */
export function useAgentAvatars(projectId: string, agentIds: readonly string[]): AgentAvatars {
  const [urls, setUrls] = useState<Record<string, string | null>>({});
  const requested = useRef(new Set<string>());
  /** Per agent, the latest request: an older answer arriving late is dropped. */
  const latest = useRef(new Map<string, number>());
  const counter = useRef(0);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const fetchOne = useCallback(
    (agentId: string) => {
      const request = ++counter.current;
      latest.current.set(agentId, request);
      void call(() => commands.agentAvatar(projectId, agentId)).then((outcome) => {
        if (!mounted.current || latest.current.get(agentId) !== request) return;
        if (!outcome.ok) console.warn(`Couldn't load the avatar of ${agentId}:`, outcome.error.message);
        setUrls((current) => ({ ...current, [agentId]: outcome.ok ? outcome.data : null }));
      });
    },
    [projectId],
  );

  const key = agentIds.join("\n");
  useEffect(() => {
    for (const agentId of key ? key.split("\n") : []) {
      if (requested.current.has(agentId)) continue;
      requested.current.add(agentId);
      fetchOne(agentId);
    }
  }, [key, fetchOne]);

  const srcOf = useCallback((agentId: string) => urls[agentId] ?? DEFAULT_AVATAR, [urls]);

  return { srcOf, refresh: fetchOne };
}
