import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type HandoffResult, type PromptFile, type TaskPromptResult } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type PromptsLoad = { status: "loading" } | { status: "error"; error: AppError } | { status: "ready"; files: PromptFile[] };

export interface Prompts {
  load: PromptsLoad;
  reload: () => void;
  /** Writes `.foundry/prompts/handoff.md`, then lists again. Nothing uses Claude. */
  generateHandoff: () => Promise<Outcome<HandoffResult>>;
  /** Writes `.foundry/prompts/<sliceId>-<agentId>.md`, then lists again. Nothing uses Claude. */
  generateTaskPrompt: (sliceId: string, agentId: string, force: boolean) => Promise<Outcome<TaskPromptResult>>;
}

/** The project's prompt files (`listPrompts`) and the two commands that write them. */
export function usePrompts(projectId: string): Prompts {
  const [load, setLoad] = useState<PromptsLoad>({ status: "loading" });
  const latestFetch = useRef(0);
  const mounted = useRef(true);

  const fetchList = useCallback(async () => {
    const request = ++latestFetch.current;
    const outcome = await call(() => commands.listPrompts(projectId));
    if (!mounted.current || request !== latestFetch.current) return;
    if (outcome.ok) setLoad({ status: "ready", files: outcome.data });
    else setLoad((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
  }, [projectId]);

  useEffect(() => {
    mounted.current = true;
    void fetchList();
    return () => {
      mounted.current = false;
    };
  }, [fetchList]);

  const reload = useCallback(() => {
    setLoad((previous) => (previous.status === "error" ? { status: "loading" } : previous));
    void fetchList();
  }, [fetchList]);

  const generateHandoff = useCallback(async () => {
    const outcome = await call(() => commands.generateHandoff(projectId));
    if (outcome.ok) await fetchList();
    return outcome;
  }, [projectId, fetchList]);

  const generateTaskPrompt = useCallback(
    async (sliceId: string, agentId: string, force: boolean) => {
      const outcome = await call(() => commands.generateTaskPrompt(projectId, sliceId, agentId, { force }));
      if (outcome.ok) await fetchList();
      return outcome;
    },
    [projectId, fetchList],
  );

  return { load, reload, generateHandoff, generateTaskPrompt };
}
