import { GitCommitHorizontal, X } from "lucide-react";
import type { AppError, CommitSetupResult } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { plural } from "../lib/order";
import { ActionError } from "./Notices";

interface SetupNotCommittedProps {
  /** The `setup_not_committed` refusal, shown as-is. */
  refusal: AppError;
  committing: boolean;
  /** A failed commit, shown as-is under the button. */
  commitError: AppError | null;
  onCommit: () => void;
}

/** A start refused because setup files aren't committed, with the fix one click away. */
export function SetupNotCommitted({ refusal, committing, commitError, onCommit }: SetupNotCommittedProps) {
  return (
    <div className="flex flex-col gap-2">
      <ActionError error={refusal} />
      <PressButton size="sm" variant="outline" className="w-fit" disabled={committing} onClick={onCommit}>
        <GitCommitHorizontal aria-hidden data-icon="inline-start" />
        {committing ? "Committing…" : "Commit setup"}
      </PressButton>
      <ActionError error={commitError} />
    </div>
  );
}

/** What "Commit setup" committed: the files and the short commit id. */
export function CommittedSetup({ result, onDismiss }: { result: CommitSetupResult; onDismiss: () => void }) {
  const short = result.commit.slice(0, 7);
  return (
    <div role="status" className="flex items-start gap-2 rounded-xl bg-muted/60 py-2 pr-1.5 pl-3 text-[13px]">
      <GitCommitHorizontal aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
      <div className="flex min-w-0 flex-1 flex-col gap-1 py-px">
        {result.files.length === 0 ? (
          <p>Nothing to commit: the setup files were already committed.</p>
        ) : (
          <>
            <p>
              Committed {plural(result.files.length, "setup file")}
              {short && (
                <>
                  {" "}
                  as <code className="font-mono text-[12px]">{short}</code>
                </>
              )}
            </p>
            <ul className="flex flex-col gap-0.5 font-mono text-[12px] text-muted-foreground">
              {result.files.map((file) => (
                <li key={file} className="truncate" title={file}>
                  {file}
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
      <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={onDismiss} className="rounded-full">
        <X aria-hidden />
      </PressButton>
    </div>
  );
}
