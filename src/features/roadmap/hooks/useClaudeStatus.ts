import { useEffect, useState } from "react";
import { commands, type AppError } from "@/lib/bindings";
import { call, type Outcome } from "./call";
import type { ClaudeStatus } from "@/lib/bindings";

export interface ClaudeReadiness {
  /** False until the first answer. */
  checked: boolean;
  /** Why generating can't run (not installed, not signed in, wrong auth mode, …), or null. */
  problem: AppError | null;
}

// Views mounting together (pane header and body) share one request.
let inFlight: Promise<Outcome<ClaudeStatus>> | null = null;

function fetchStatus() {
  inFlight ??= call(() => commands.claudeStatus()).finally(() => {
    inFlight = null;
  });
  return inFlight;
}

/** Asks the backend whether Claude Code can run, on every mount (sign-in can change). */
export function useClaudeStatus(): ClaudeReadiness {
  const [readiness, setReadiness] = useState<ClaudeReadiness>({ checked: false, problem: null });

  useEffect(() => {
    let cancelled = false;
    fetchStatus().then((outcome) => {
      if (cancelled) return;
      setReadiness({ checked: true, problem: outcome.ok ? outcome.data.problem : outcome.error });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return readiness;
}
