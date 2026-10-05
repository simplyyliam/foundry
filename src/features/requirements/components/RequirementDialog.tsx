import { useState, type KeyboardEvent, type ReactNode } from "react";
import { Info } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { PlanRequirement, RequirementKind, SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { RequirementActions } from "../hooks/useRequirementActions";
import { ActionError } from "./Notices";
import { SliceMenu } from "./SliceMenu";

const KINDS: RequirementKind[] = ["behavior", "interaction", "constraint"];

interface RequirementDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: RequirementActions;
  /** In Blueprint order (for picking a slice when adding). */
  slices: SliceView[];
  /** Edit this requirement, or add a new one when absent. */
  requirement?: PlanRequirement;
}

/** Add a requirement, or edit one's text, kind, excerpt and how it's verified. */
export function RequirementDialog({ open, onOpenChange, ...form }: RequirementDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" showCloseButton={false}>
        {/* Mounts with the dialog, so every opening starts from the saved values. */}
        <RequirementForm {...form} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function RequirementForm({
  actions,
  slices,
  requirement,
  onClose,
}: Omit<RequirementDialogProps, "open" | "onOpenChange"> & { onClose: () => void }) {
  const [text, setText] = useState(requirement?.text ?? "");
  const [kind, setKind] = useState<RequirementKind>(requirement?.kind ?? "behavior");
  const [excerpt, setExcerpt] = useState(requirement?.sourceExcerpt ?? "");
  const [verifiedBy, setVerifiedBy] = useState(requirement?.verifiedBy ?? "");
  const [sliceId, setSliceId] = useState<string | null>(null);
  const { busy, error, run } = useAction();

  const editing = requirement !== undefined;
  // Compared as typed, so opening and saving untouched fields never counts as a change.
  const changed = {
    text: editing && text !== requirement.text,
    kind: editing && kind !== requirement.kind,
    excerpt: editing && excerpt !== requirement.sourceExcerpt,
    verifiedBy: editing && verifiedBy !== requirement.verifiedBy,
  };
  const anyChange = changed.text || changed.kind || changed.excerpt || changed.verifiedBy;
  const resetsToProposed = editing && requirement.state === "confirmed" && (changed.text || changed.kind || changed.excerpt);

  const submit = async () => {
    if (busy) return;
    if (editing && !anyChange) return onClose();
    const outcome = editing
      ? await run(() =>
          actions.update(requirement.id, {
            text: changed.text ? text.trim() : null,
            kind: changed.kind ? kind : null,
            sourceExcerpt: changed.excerpt ? excerpt.trim() : null,
            verifiedBy: changed.verifiedBy ? verifiedBy.trim() : null,
          }),
        )
      : await run(() =>
          actions.add({ text: text.trim(), kind, sliceId, sourceExcerpt: excerpt.trim(), verifiedBy: verifiedBy.trim() }),
        );
    if (outcome.ok) onClose();
  };

  const submitOnModEnter = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void submit();
    }
  };

  return (
    <form
      className="contents"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <DialogHeader>
        <DialogTitle>{editing ? "Edit requirement" : "Add requirement"}</DialogTitle>
        <DialogDescription>
          {editing ? "Ctrl + Enter saves." : "It's added as proposed. Ctrl + Enter adds it."}
        </DialogDescription>
      </DialogHeader>

      <div className="flex flex-col gap-4">
        <Field id="requirement-text" label="Requirement">
          <Textarea
            id="requirement-text"
            autoFocus
            value={text}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={submitOnModEnter}
            placeholder="What the product does, in one sentence."
            className="min-h-16 text-[13px]"
          />
        </Field>

        <div className="flex flex-wrap items-end gap-4">
          <Field label="Kind">
            <div role="radiogroup" aria-label="Kind" className="flex w-fit gap-0.5 rounded-full bg-muted p-0.5">
              {KINDS.map((option) => (
                <PressButton
                  key={option}
                  type="button"
                  role="radio"
                  aria-checked={kind === option}
                  pressScale={0.95}
                  size="xs"
                  variant="ghost"
                  onClick={() => setKind(option)}
                  className={cn(
                    "rounded-full px-3",
                    kind === option ? "bg-background text-foreground shadow-sm hover:bg-background" : "text-muted-foreground",
                  )}
                >
                  {option}
                </PressButton>
              ))}
            </div>
          </Field>
          {!editing && (
            <Field label="Slice">
              <SliceMenu slices={slices} value={sliceId} onChange={setSliceId} className="h-7" />
            </Field>
          )}
        </div>

        <Field id="requirement-excerpt" label="From the brain dump" hint="The words it comes from. Without a match it's marked unsourced.">
          <Textarea
            id="requirement-excerpt"
            value={excerpt}
            onChange={(event) => setExcerpt(event.target.value)}
            onKeyDown={submitOnModEnter}
            className="min-h-12 text-[13px]"
          />
        </Field>

        <Field id="requirement-verified" label="Verified by" hint="How someone checks it's done.">
          <Textarea
            id="requirement-verified"
            value={verifiedBy}
            onChange={(event) => setVerifiedBy(event.target.value)}
            onKeyDown={submitOnModEnter}
            className="min-h-12 text-[13px]"
          />
        </Field>

        {resetsToProposed && (
          <p role="status" className="flex items-start gap-2 rounded-xl bg-muted px-3 py-2 text-[13px]">
            <Info aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
            This requirement is confirmed. Changing its {describeChange(changed)} moves it back to proposed when you save.
          </p>
        )}
        <ActionError error={error} />
      </div>

      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="submit" disabled={busy || text.trim() === ""}>
          {editing ? (resetsToProposed ? "Save as proposed" : "Save") : "Add requirement"}
        </PressButton>
      </DialogFooter>
    </form>
  );
}

function describeChange(changed: { text: boolean; kind: boolean; excerpt: boolean }) {
  const parts = [changed.text && "text", changed.kind && "kind", changed.excerpt && "excerpt"].filter(Boolean);
  return parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}` : parts[0];
}

function Field({ id, label, hint, children }: { id?: string; label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      {id ? (
        <Label htmlFor={id} className="text-[13px]">
          {label}
        </Label>
      ) : (
        <span className="text-[13px] leading-none font-medium">{label}</span>
      )}
      {children}
      {hint && <p className="text-[12px] text-muted-foreground">{hint}</p>}
    </div>
  );
}
