import { FileCheck2, LoaderCircle, TriangleAlert } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { GenerateAgentsResult } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { AgentsSetup } from "../hooks/useAgentsSetup";

interface GenerateAgentsBlockProps {
  generate: AgentsSetup["generate"];
  /** Labels still to confirm (or being edited). Generate is disabled while there are any. */
  pending: string[];
  disabled: boolean;
  /** The last generation's result, kept by the section (cleared when a value changes). */
  result: GenerateAgentsResult | null;
  onResult: (result: GenerateAgentsResult) => void;
}

/**
 * "Generate AGENTS.md". A differing AGENTS.md comes back as `agents_md_exists`: a dialog
 * asks before sending `overwrite: true` (the backend backs the old file up first).
 */
export function GenerateAgentsBlock({ generate, pending, disabled, result, onResult }: GenerateAgentsBlockProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const blocked = disabled || pending.length > 0;

  const run = async () => {
    setBusy(true);
    setError(null);
    const outcome = await generate(false);
    setBusy(false);
    if (outcome.ok) onResult(outcome.data);
    else if (outcome.error.code === "agents_md_exists") setAsking(true);
    else if (outcome.error.code !== "folder_missing") setError(outcome.error.message);
  };

  return (
    <div className="flex flex-col gap-3 border-t border-border/60 pt-5">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <PressButton disabled={blocked || busy} onClick={() => void run()}>
          {busy && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
          Generate AGENTS.md
        </PressButton>
        {!disabled && pending.length > 0 && (
          <p className="min-w-0 flex-1 text-[12.5px] leading-snug text-muted-foreground">Confirm {listOf(pending)} first.</p>
        )}
      </div>

      {error && (
        <p role="alert" className="flex items-start gap-2 rounded-lg bg-destructive/8 px-3 py-2 text-[12.5px]">
          <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
          {error}
        </p>
      )}
      {result && <GenerateOutcome result={result} />}

      <Dialog open={asking} onOpenChange={setAsking}>
        <DialogContent className="sm:max-w-md" showCloseButton={false}>
          {/* Mounts with the dialog: fresh state each time, and press feedback can attach. */}
          <ReplaceBody
            onClose={() => setAsking(false)}
            onReplace={async () => {
              const outcome = await generate(true);
              if (outcome.ok) onResult(outcome.data);
              return outcome.ok ? null : outcome.error.message;
            }}
          />
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ReplaceBody({ onClose, onReplace }: { onClose: () => void; onReplace: () => Promise<string | null> }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const replace = async () => {
    setBusy(true);
    setError(null);
    const message = await onReplace();
    setBusy(false);
    if (message === null) onClose();
    else setError(message);
  };

  return (
    <>
      <DialogHeader>
        <DialogTitle>Replace AGENTS.md?</DialogTitle>
        <DialogDescription className="text-[13px] leading-snug">
          AGENTS.md already exists and differs. Replace it? A backup is saved to .foundry/backups/ first.
        </DialogDescription>
      </DialogHeader>
      {error && (
        <p role="alert" className="rounded-lg bg-destructive/8 px-3 py-2 text-[13px]">
          {error}
        </p>
      )}
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="button" autoFocus disabled={busy} onClick={() => void replace()}>
          Replace
        </PressButton>
      </DialogFooter>
    </>
  );
}

/** What the result says, field by field. Lists of names come from the backend as-is. */
function GenerateOutcome({ result }: { result: GenerateAgentsResult }) {
  return (
    <div role="status" className="flex flex-col gap-2 rounded-xl bg-muted/60 p-4 text-[13px] ring-1 ring-foreground/5">
      <p className="flex items-center gap-2 font-medium">
        <FileCheck2 aria-hidden className="size-4 text-primary" />
        {result.agentsMdWritten ? "Wrote AGENTS.md" : "AGENTS.md already had this exact text, so it wasn't rewritten."}
      </p>
      {result.designCopySkipped && (
        <p className="text-[12.5px] text-muted-foreground">
          No Design Library file matches the family, so agents will rely on tokens and the References.
        </p>
      )}
      <dl className="flex flex-col gap-1.5 text-[12.5px] empty:hidden">
        {result.backup && <Line term="Previous version backed up to">{code(result.backup)}</Line>}
        {result.designFamilies.length > 0 && <Line term="Design families">{result.designFamilies.join(", ")}</Line>}
        {!result.designCopySkipped && result.copied.length > 0 && (
          <Line term="Copied into docs/design/">{result.copied.map(code)}</Line>
        )}
        {result.unmatchedFamilies.length > 0 && (
          <Line term="No library file is tagged">{result.unmatchedFamilies.join(", ")}</Line>
        )}
        {result.removed.length > 0 && <Line term="Removed earlier copies that no longer match">{result.removed.map(code)}</Line>}
      </dl>
    </div>
  );
}

function Line({ term, children }: { term: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-baseline gap-x-1.5 gap-y-1">
      <dt className="text-muted-foreground">{term}:</dt>
      <dd className="flex flex-wrap gap-1">{children}</dd>
    </div>
  );
}

const code = (text: string) => (
  <code key={text} className="rounded bg-background px-1 py-px font-mono text-[12px] ring-1 ring-foreground/10">
    {text}
  </code>
);

function listOf(items: string[]): string {
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}
