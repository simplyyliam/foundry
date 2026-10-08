import { LoaderCircle, Square } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import { cancelVerification, type RunProgress } from "../hooks/verifyRuns";
import { useElapsed } from "../hooks/useElapsed";
import { formatElapsed, STAGE_LABEL } from "../lib/verification";
import { ErrorLine } from "./Notices";

/** Stage, the command running and elapsed time from `verify:progress`, plus Cancel. */
export function ProgressLine({ projectId, run, className }: { projectId: string; run: RunProgress; className?: string }) {
  const elapsed = useElapsed(run.elapsedMs, run.receivedAt, true);
  const [cancel, setCancel] = useState<{ busy: boolean; error: string | null }>({ busy: false, error: null });

  const onCancel = async () => {
    setCancel({ busy: true, error: null });
    const outcome = await cancelVerification(projectId);
    setCancel({ busy: false, error: outcome.ok ? null : outcome.error.message });
  };

  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      <div role="status" className="flex items-center gap-2 rounded-lg bg-muted/70 py-1 pr-1 pl-2.5 text-[12.5px]">
        <LoaderCircle aria-hidden className="size-3.5 shrink-0 animate-spin text-muted-foreground" />
        <span className="shrink-0 font-medium">{STAGE_LABEL[run.stage]}</span>
        {run.command && (
          <code className="min-w-0 truncate font-mono text-[11.5px] text-muted-foreground" title={run.command}>
            {run.command}
          </code>
        )}
        <span className="ml-auto shrink-0 text-muted-foreground tabular-nums">{formatElapsed(elapsed)}</span>
        <PressButton size="xs" variant="ghost" disabled={cancel.busy} onClick={() => void onCancel()} className="shrink-0">
          <Square aria-hidden data-icon="inline-start" />
          Cancel
        </PressButton>
      </div>
      {cancel.error && <ErrorLine message={cancel.error} />}
    </div>
  );
}
