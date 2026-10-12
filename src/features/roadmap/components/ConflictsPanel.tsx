import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { AnimatePresence, motion, useReducedMotionConfig } from "motion/react";
import { CircleCheck, PenLine, RotateCw, Scale, TriangleAlert, Undo2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import type { AppError, ConflictSource, RoadmapConflict, RulingKind } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import type { Outcome } from "../hooks/call";
import type { ConflictsController } from "../hooks/useConflicts";

const STALE_TEXT = "The plan changed elsewhere, so it was reloaded. Try again.";

const FOLLOW: Record<Exclude<RulingKind, "custom">, string> = {
  reference_wins: "Follow the reference",
  brain_dump_wins: "Follow the brain dump",
};

interface ConflictsPanelProps {
  controller: ConflictsController;
  /** Changes when a page asks to settle the conflicts: scroll here and focus the heading. */
  focusToken?: number;
}

/**
 * "Conflicts to settle": where the brain dump and a reference disagree. Open cards take a
 * ruling; settled ones fold to one line with Undo. Hidden when the plan has no conflicts.
 */
export function ConflictsPanel({ controller, focusToken }: ConflictsPanelProps) {
  const { data } = controller;
  const headingId = useId();
  const headingRef = useRef<HTMLHeadingElement>(null);
  const unsettled = data.conflicts.filter((conflict) => !conflict.ruling).length;
  const shown = data.conflicts.length > 0;

  useEffect(() => {
    if (!focusToken || !shown) return;
    headingRef.current?.scrollIntoView({ block: "start" });
    headingRef.current?.focus({ preventScroll: true });
  }, [focusToken, shown]);

  if (!shown) return data.error && data.status === "error" ? <ErrorText error={data.error} /> : null;

  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2.5">
      <div className="flex items-center gap-2">
        <h3 id={headingId} ref={headingRef} tabIndex={-1} className="text-[13px] leading-snug font-medium outline-none">
          Conflicts to settle
        </h3>
        {unsettled > 0 ? (
          <Badge variant="secondary" className="h-4.5 min-w-4.5 px-1.5 text-[10.5px] tabular-nums" aria-label={`${unsettled} unsettled`}>
            {unsettled}
          </Badge>
        ) : (
          <span className="text-[11.5px] text-muted-foreground">All settled</span>
        )}
      </div>
      <p className="-mt-1 text-[12px] leading-snug text-muted-foreground">
        Your brain dump and your references disagree here. Pick which one agents follow.
      </p>
      {data.error && <ErrorText error={data.error} />}
      <ul className="flex flex-col gap-2">
        {data.conflicts.map((conflict) => (
          <ConflictItem key={conflict.id} conflict={conflict} controller={controller} />
        ))}
      </ul>
    </section>
  );
}

function ConflictItem({ conflict, controller }: { conflict: RoadmapConflict; controller: ConflictsController }) {
  // Kept across Undo, so reopening a card remembers the last choice.
  const [addAsRule, setAddAsRule] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const reducedMotion = useReducedMotionConfig();

  const run = async (action: () => Promise<Outcome<unknown>>) => {
    setBusy(true);
    setError(null);
    const outcome = await action();
    setBusy(false);
    if (!outcome.ok) setError(outcome.error);
    return outcome;
  };

  const ruling = conflict.ruling ?? null;
  return (
    <li className="rounded-xl ring-1 ring-foreground/8">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={ruling ? "settled" : "open"}
          initial={{ opacity: 0, filter: reducedMotion ? "blur(0px)" : "blur(2px)" }}
          animate={{ opacity: 1, filter: "blur(0px)" }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
        >
          {ruling ? (
            <SettledLine
              conflict={conflict}
              busy={busy}
              onUndo={() => void run(() => controller.unresolve(conflict.id))}
            />
          ) : (
            <OpenCard
              conflict={conflict}
              busy={busy}
              addAsRule={addAsRule}
              onAddAsRuleChange={setAddAsRule}
              onRule={(kind, text) => run(() => controller.resolve(conflict.id, { kind, text: text ?? null }, addAsRule))}
            />
          )}
        </motion.div>
      </AnimatePresence>
      {error && <ErrorText error={error} className="mx-3 mb-3" />}
    </li>
  );
}

function OpenCard({
  conflict,
  busy,
  addAsRule,
  onAddAsRuleChange,
  onRule,
}: {
  conflict: RoadmapConflict;
  busy: boolean;
  addAsRule: boolean;
  onAddAsRuleChange: (value: boolean) => void;
  onRule: (kind: RulingKind, text?: string) => Promise<Outcome<unknown>>;
}) {
  const [writing, setWriting] = useState(false);
  const [text, setText] = useState("");
  const textId = useId();
  const follows = [...(["reference_wins", "brain_dump_wins"] as const)].sort(
    // The suggested ruling comes first.
    (a, b) => Number(b === conflict.suggested) - Number(a === conflict.suggested),
  );

  const submitOwn = () => {
    if (busy || text.trim() === "") return;
    void onRule("custom", text.trim());
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      submitOwn();
    } else if (event.key === "Escape") {
      // Closes the text box, not the pop-out around it.
      event.preventDefault();
      event.stopPropagation();
      setWriting(false);
    }
  };

  return (
    <div role="group" aria-label={conflict.topic} className="flex flex-col gap-3 p-3">
      <p className="flex items-start gap-1.5 text-[13px] leading-snug font-medium">
        <Scale aria-hidden className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
        {conflict.topic}
      </p>
      <div className="grid gap-2 @md:grid-cols-2">
        <Excerpt label="Brain dump" source="docs/brain-dump.md" text={conflict.brainDumpExcerpt} />
        <Excerpt label={sourceLabel(conflict.otherSource)} source={conflict.otherSource.ref} text={conflict.otherExcerpt} />
      </div>

      <div className="flex flex-wrap gap-1.5">
        {follows.map((kind) => {
          const suggested = kind === conflict.suggested;
          return (
            <PressButton
              key={kind}
              size="sm"
              variant={suggested ? "default" : "secondary"}
              disabled={busy}
              onClick={() => void onRule(kind)}
              className="h-auto min-h-8 py-1"
            >
              {FOLLOW[kind]}
              {suggested && <span className="rounded-sm bg-primary-foreground/15 px-1 text-[10.5px] font-normal">Suggested</span>}
            </PressButton>
          );
        })}
        <PressButton
          size="sm"
          variant="ghost"
          aria-expanded={writing}
          aria-controls={writing ? textId : undefined}
          disabled={busy}
          onClick={() => setWriting((open) => !open)}
        >
          <PenLine aria-hidden data-icon="inline-start" />
          Write my own
        </PressButton>
      </div>

      {writing && (
        <div className="flex flex-col gap-1.5">
          <Textarea
            id={textId}
            autoFocus
            aria-label="Your ruling"
            value={text}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={onKeyDown}
            placeholder="e.g. Keep the minimap, but only on screens wider than 1200 px."
            className="min-h-16 text-[12.5px] md:text-[12.5px]"
          />
          <div className="flex justify-end gap-1.5">
            <PressButton size="xs" variant="ghost" onClick={() => setWriting(false)}>
              Cancel
            </PressButton>
            <PressButton size="xs" disabled={busy || text.trim() === ""} onClick={submitOwn}>
              Save ruling
            </PressButton>
          </div>
        </div>
      )}

      <label className="flex w-fit items-center gap-2 text-[12.5px]">
        <Checkbox checked={addAsRule} disabled={busy} onCheckedChange={(checked) => onAddAsRuleChange(checked === true)} />
        Add as a project-wide rule
      </label>
    </div>
  );
}

function Excerpt({ label, source, text }: { label: string; source: string; text: string }) {
  return (
    <figure className="flex min-w-0 flex-col gap-1 rounded-lg bg-muted/60 px-2.5 py-2">
      <figcaption className="flex min-w-0 items-baseline gap-1 text-[11px] text-muted-foreground">
        <span className="shrink-0 font-medium text-foreground/80">{label}</span>
        <span aria-hidden>·</span>
        <span className="truncate font-mono text-[10.5px]" title={source}>
          {source}
        </span>
      </figcaption>
      <blockquote className="text-[12.5px] leading-snug text-foreground/85 italic select-text">{text}</blockquote>
    </figure>
  );
}

function SettledLine({ conflict, busy, onUndo }: { conflict: RoadmapConflict; busy: boolean; onUndo: () => void }) {
  const ruling = conflict.ruling!;
  const said = ruling.kind === "custom" ? `“${ruling.text ?? ""}”` : FOLLOW[ruling.kind];
  const summary = `${conflict.topic}: ${said}${conflict.ruleRequirementId ? " (project-wide rule)" : ""}`;
  return (
    <div role="group" aria-label={conflict.topic} className="flex items-center gap-2 py-1 pr-1 pl-3">
      <CircleCheck aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
      <p className="min-w-0 flex-1 truncate text-[12.5px]" title={summary}>
        <span className="font-medium">{conflict.topic}</span>
        <span className="text-muted-foreground">: {said}</span>
        {conflict.ruleRequirementId && <span className="text-muted-foreground"> · rule</span>}
      </p>
      <PressButton size="xs" variant="ghost" disabled={busy} onClick={onUndo} className="shrink-0 text-muted-foreground">
        <Undo2 aria-hidden data-icon="inline-start" />
        Undo
      </PressButton>
    </div>
  );
}

function sourceLabel(source: ConflictSource) {
  return source.kind === "bakery" ? "Bakery" : "Design file";
}

function ErrorText({ error, className }: { error: AppError; className?: string }) {
  const stale = error.code === "stale_revision";
  const Icon = stale ? RotateCw : TriangleAlert;
  return (
    <div role="alert" className={cn("flex items-start gap-2 rounded-lg bg-destructive/8 px-2.5 py-2 text-[12px]", className)}>
      <Icon aria-hidden className={cn("mt-px size-3.5 shrink-0", stale ? "text-muted-foreground" : "text-destructive")} />
      <div className="min-w-0 flex-1 leading-snug">
        {stale ? (
          <>
            <p>{STALE_TEXT}</p>
            <p className="text-muted-foreground">{error.message}</p>
          </>
        ) : (
          <p>{error.message}</p>
        )}
      </div>
    </div>
  );
}
