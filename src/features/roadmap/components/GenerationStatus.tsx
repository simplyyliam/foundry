import { LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppError, Stage } from "@/lib/bindings";
import { Button } from "@/components/ui/button";
import { usePressScale } from "@/shared";
import type { GenerationState } from "../hooks/useGeneration";
import { Artifact } from "./Artifact";

const STAGE_LABEL: Record<Stage, string> = {
  preflight: "Checking Claude Code",
  running: "Generating the roadmap",
  validating: "Checking the result",
  retrying: "Retrying",
  checking_conflicts: "Comparing with your references",
  done: "Finishing",
  failed: "Stopping",
};

/** Re-renders every second while mounted. */
function useNow() {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

function formatElapsed(ms: number) {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

/** Stage and elapsed time of a running generation (counted up locally between events), with Cancel. */
export function GenerationProgress({ state, onCancel }: { state: GenerationState; onCancel: () => void }) {
  const now = useNow();
  const cancelRef = usePressScale<HTMLButtonElement>();
  const { progress, phase } = state;

  const label =
    phase === "loading" ? "Loading into the plan" : progress ? STAGE_LABEL[progress.stage] : "Starting";
  const elapsed = progress ? progress.elapsedMs + (now - progress.receivedAt) : null;
  // Cancel stops the generation; once it's loading into the plan there's nothing to cancel.
  const cancellable = phase === "generating";

  return (
    <Artifact>
      <div className="flex items-center gap-2.5">
        <LoaderCircle aria-hidden className="size-4 shrink-0 animate-spin text-muted-foreground" />
        <div className="min-w-0 flex-1" role="status" aria-live="polite">
          <p className="truncate text-[13px] leading-snug text-foreground">{label}…</p>
          {elapsed !== null && (
            <p className="text-[12px] leading-snug text-muted-foreground tabular-nums">{formatElapsed(elapsed)}</p>
          )}
        </div>
        {cancellable && (
          <Button ref={cancelRef} size="xs" variant="outline" onClick={onCancel}>
            Cancel
          </Button>
        )}
      </div>
    </Artifact>
  );
}

/** Backend errors, exactly as returned. */
export function ErrorArtifact({ errors }: { errors: readonly AppError[] }) {
  if (errors.length === 0) return null;
  return (
    <Artifact>
      <div role="alert" className="flex flex-col gap-1">
        {errors.map((error, index) => (
          <p key={index} className="text-[13px] leading-snug text-destructive">
            {error.message}
          </p>
        ))}
      </div>
    </Artifact>
  );
}
