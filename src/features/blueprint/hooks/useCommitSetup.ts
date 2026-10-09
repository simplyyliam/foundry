import { useCallback } from "react";
import type { CommitSetupResult } from "@/lib/bindings";
import type { Outcome } from "./call";
import { useAction } from "./useAction";

/**
 * "Commit setup" after a start was refused with `setup_not_committed`: commits, hands the
 * result to `onCommitted`, then runs `retry` (the refused move again). A failed commit keeps
 * its error (the backend's, as-is, e.g. `git_identity_missing` with how to fix it) and
 * doesn't retry.
 */
export function useCommitSetup(
  commit: () => Promise<Outcome<CommitSetupResult>>,
  onCommitted: (result: CommitSetupResult) => void,
) {
  const { busy, error, run, clear } = useAction();

  const commitThen = useCallback(
    async (retry: () => Promise<unknown>) => {
      if (busy) return;
      const outcome = await run(commit);
      if (!outcome.ok) return;
      onCommitted(outcome.data);
      await retry();
    },
    [busy, run, commit, onCommitted],
  );

  return { committing: busy, error, commitThen, clear };
}
