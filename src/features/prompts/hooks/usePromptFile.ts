import { useCallback, useEffect, useState } from "react";
import { commands } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export type PromptText = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; text: string };

/** One prompt file's text (`readPrompt`), read when the sheet shows it (keyed by file). */
export function usePromptText(projectId: string, file: string): PromptText {
  const [text, setText] = useState<PromptText>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    void call(() => commands.readPrompt(projectId, file)).then((outcome) => {
      if (cancelled) return;
      setText(outcome.ok ? { status: "ready", text: outcome.data } : { status: "error", message: outcome.error.message });
    });
    return () => {
      cancelled = true;
    };
  }, [projectId, file]);

  return text;
}

/** The one line that starts an agent's session on a prompt file (`launchMessage`). */
export function useLaunchMessage(projectId: string): (agentId: string, file: string) => Promise<Outcome<string>> {
  return useCallback(
    (agentId: string, file: string) => call(() => commands.launchMessage(projectId, agentId, file)),
    [projectId],
  );
}
