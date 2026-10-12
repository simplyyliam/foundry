import { useCallback, useState } from "react";
import { commands, type CommitSetupResult } from "@/lib/bindings";
import { call } from "./call";
import { useAction } from "./useAction";

/**
 * Commits Foundry's setup files (`commitSetup`), for the guidance's "Commit the setup". The
 * result (or the refusal, as-is) stays until dismissed or the next try.
 */
export function useCommitSetup(projectId: string) {
  const action = useAction();
  const [result, setResult] = useState<CommitSetupResult | null>(null);
  const { run, clear } = action;

  const commit = useCallback(async () => {
    setResult(null);
    const outcome = await run(() => call(() => commands.commitSetup(projectId)));
    if (outcome.ok) setResult(outcome.data);
    return outcome;
  }, [projectId, run]);

  const dismiss = useCallback(() => {
    setResult(null);
    clear();
  }, [clear]);

  return { busy: action.busy, error: action.error, result, commit, dismiss };
}
