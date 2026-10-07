import { FileCheck2, LoaderCircle } from "lucide-react";
import { useState } from "react";
import type { AppError, HandoffResult } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { Prompts } from "../hooks/usePrompts";
import { GateError, WarningNotice } from "./Notices";
import { PromptCard } from "./PromptCard";

interface HandoffCardProps {
  projectId: string;
  /** A hand-off file exists already: the button says Regenerate. */
  exists: boolean;
  generate: Prompts["generateHandoff"];
  /** Opens a written file in the sheet. */
  onOpen: (path: string) => void;
}

/** "Generate hand-off": the brief every agent reads first. */
export function HandoffCard({ projectId, exists, generate, onOpen }: HandoffCardProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [result, setResult] = useState<HandoffResult | null>(null);

  const run = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    const outcome = await generate();
    setBusy(false);
    if (outcome.ok) setResult(outcome.data);
    else setError(outcome.error);
  };

  return (
    <PromptCard title="Hand-off" description="The project brief every agent reads: profile, stack, and requirements by slice.">
      <div>
        <PressButton disabled={busy} onClick={() => void run()}>
          {busy && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
          {exists ? "Regenerate hand-off" : "Generate hand-off"}
        </PressButton>
      </div>
      {error && <GateError projectId={projectId} error={error} />}
      {result && (
        <div className="flex flex-col gap-2">
          <Written path={result.path} onOpen={() => onOpen(result.path)} />
          <WarningNotice warnings={result.warnings} />
        </div>
      )}
    </PromptCard>
  );
}

/** "Wrote <path>" with an Open button, after a generation. */
export function Written({ path, onOpen }: { path: string; onOpen: () => void }) {
  return (
    <div role="status" className="flex items-center gap-2 text-[12.5px]">
      <FileCheck2 aria-hidden className="size-3.5 shrink-0 text-primary" />
      <p className="min-w-0 truncate">
        Wrote <span className="font-mono text-[12px]">{path}</span>
      </p>
      <PressButton size="xs" variant="ghost" className="shrink-0 rounded-md text-muted-foreground" onClick={onOpen}>
        Open
      </PressButton>
    </div>
  );
}
