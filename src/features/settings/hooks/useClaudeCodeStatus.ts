import { useCallback, useEffect, useState } from "react";
import { commands, type ClaudeStatus } from "@/lib/bindings";
import { call } from "./call";

export type ClaudeStatusState =
  | { status: "checking"; last: ClaudeStatus | null }
  | { status: "ready"; data: ClaudeStatus }
  | { status: "error"; message: string };

/**
 * `claudeStatus` on mount and on `refresh()`. It only checks the CLI and its login, so it
 * never uses the subscription.
 */
export function useClaudeCodeStatus(): { state: ClaudeStatusState; refresh: () => void } {
  const [state, setState] = useState<ClaudeStatusState>({ status: "checking", last: null });
  const [run, setRun] = useState(0);

  useEffect(() => {
    let cancelled = false;
    call(() => commands.claudeStatus()).then((outcome) => {
      if (cancelled) return;
      setState(outcome.ok ? { status: "ready", data: outcome.data } : { status: "error", message: outcome.error.message });
    });
    return () => {
      cancelled = true;
    };
  }, [run]);

  const refresh = useCallback(() => {
    // Keep the last answer on screen while checking again, so the rows don't jump.
    setState((current) => ({ status: "checking", last: current.status === "ready" ? current.data : null }));
    setRun((n) => n + 1);
  }, []);

  return { state, refresh };
}
