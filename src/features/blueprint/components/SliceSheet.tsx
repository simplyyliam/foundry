import { useEffect, useRef, useState, type ReactNode } from "react";
import { useReducedMotionConfig } from "motion/react";
import { Pencil, Trash2 } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { Plan, SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { plural } from "../lib/order";
import { ActionError } from "./Notices";
import { SliceDependencies } from "./SliceDependencies";
import { SliceLaterItems } from "./SliceLaterItems";
import { SliceDecisions, SliceQuestions } from "./SliceNotes";
import { SliceStatusControl } from "./SliceStatusControl";
import { StatusChip } from "./StatusChip";

interface SliceSheetProps {
  plan: Plan;
  /** The slice to show, or null when closed. */
  sliceId: string | null;
  actions: PlanActions;
  onClose: () => void;
}

/** A slice's details and every edit to it, opened by clicking its box. */
export function SliceSheet({ plan, sliceId, actions, onClose }: SliceSheetProps) {
  // Keep showing the last slice while the sheet animates out.
  const [shown, setShown] = useState<string | null>(sliceId);
  if (sliceId !== null && sliceId !== shown) setShown(sliceId);
  const slice = plan.slices.find((candidate) => candidate.id === shown) ?? null;

  return (
    <Sheet open={sliceId !== null && slice !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full gap-0 sm:max-w-md">
        {slice && <SliceDetails key={slice.id} plan={plan} slice={slice} actions={actions} onRemoved={onClose} />}
      </SheetContent>
    </Sheet>
  );
}

interface SliceDetailsProps {
  plan: Plan;
  slice: SliceView;
  actions: PlanActions;
  onRemoved: () => void;
}

function SliceDetails({ plan, slice, actions, onRemoved }: SliceDetailsProps) {
  const count = plan.requirements.filter((requirement) => requirement.sliceId === slice.id).length;

  return (
    <>
      <SheetHeader className="gap-2 pr-14">
        <SliceName slice={slice} actions={actions} />
        <SheetDescription className="flex items-center gap-2 text-[13px]">
          <StatusChip status={slice.status} />
          <span>{plural(count, "requirement")}</span>
          {slice.ready && <span className="text-slice-foreground">· ready to start</span>}
        </SheetDescription>
      </SheetHeader>
      <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-6 pb-6">
        <Section title="Status">
          <SliceStatusControl plan={plan} slice={slice} actions={actions} />
        </Section>
        <Section title="Delivers">
          {slice.delivers.length === 0 ? (
            <p className="text-[13px] text-muted-foreground">Nothing listed.</p>
          ) : (
            <ul className="flex list-disc flex-col gap-1 pl-5 text-[13px] leading-snug marker:text-muted-foreground">
              {slice.delivers.map((item, index) => (
                <li key={`${index}:${item}`}>{item}</li>
              ))}
            </ul>
          )}
        </Section>
        <Section title="Depends on">
          <SliceDependencies plan={plan} slice={slice} actions={actions} />
        </Section>
        <Section title="Decisions">
          <SliceDecisions slice={slice} actions={actions} />
        </Section>
        <Section title="Open questions">
          <SliceQuestions slice={slice} actions={actions} />
        </Section>
        <Section title="Later items">
          <SliceLaterItems plan={plan} slice={slice} actions={actions} />
        </Section>
        <DeleteSlice slice={slice} actions={actions} onRemoved={onRemoved} />
      </div>
    </>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-[12px] font-medium tracking-wide text-muted-foreground uppercase">{title}</h3>
      {children}
    </section>
  );
}

/** The title, editable in place: Enter saves, Escape cancels. */
function SliceName({ slice, actions }: { slice: SliceView; actions: PlanActions }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(slice.name);
  const { busy, error, run, clear } = useAction();

  const cancel = () => {
    setEditing(false);
    setName(slice.name);
    clear();
  };

  if (!editing) {
    return (
      <div className="flex items-start gap-1">
        <SheetTitle className="min-w-0 text-lg leading-snug text-pretty">{slice.name}</SheetTitle>
        <PressButton
          variant="ghost"
          size="icon-xs"
          aria-label="Rename slice"
          title="Rename"
          className="mt-0.5 shrink-0 rounded-full text-muted-foreground"
          onClick={() => {
            setName(slice.name);
            setEditing(true);
          }}
        >
          <Pencil aria-hidden />
        </PressButton>
      </div>
    );
  }

  return (
    <form
      className="flex flex-col gap-2"
      onSubmit={async (event) => {
        event.preventDefault();
        if (busy) return;
        if (name.trim() === slice.name) return cancel();
        if ((await run(() => actions.renameSlice(slice.id, name.trim()))).ok) setEditing(false);
      }}
    >
      <SheetTitle className="sr-only">{slice.name}</SheetTitle>
      <div className="flex items-center gap-1.5">
        <Input
          autoFocus
          value={name}
          aria-label="Slice name"
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              // Cancel the rename, not the whole sheet.
              event.stopPropagation();
              event.preventDefault();
              cancel();
            }
          }}
          className="h-9 text-base font-medium"
        />
        <PressButton type="submit" size="sm" disabled={busy}>
          Save
        </PressButton>
      </div>
      <ActionError error={error} />
    </form>
  );
}

/**
 * Delete, with one confirming click. The backend refuses while requirements, dependent slices
 * or deferred items still point at the slice; its message says which (shown as-is). Later
 * items are fixed in the section right above.
 */
function DeleteSlice({ slice, actions, onRemoved }: { slice: SliceView; actions: PlanActions; onRemoved: () => void }) {
  const [confirming, setConfirming] = useState(false);
  const { busy, error, run } = useAction();
  const sectionRef = useRef<HTMLElement>(null);
  const reducedMotion = useReducedMotionConfig();

  // A refusal lands at the very bottom of the sheet: bring it into view.
  useEffect(() => {
    if (error) sectionRef.current?.scrollIntoView({ block: "nearest", behavior: reducedMotion ? "auto" : "smooth" });
  }, [error, reducedMotion]);

  return (
    <section ref={sectionRef} className="mt-2 flex flex-col gap-2 border-t border-border/60 pt-4">
      {confirming ? (
        <div className="flex flex-wrap items-center gap-2">
          <p className="mr-auto text-[13px]">Delete “{slice.name}”?</p>
          <PressButton variant="ghost" size="sm" onClick={() => setConfirming(false)}>
            Cancel
          </PressButton>
          <PressButton
            variant="destructive"
            size="sm"
            disabled={busy}
            onClick={async () => {
              const outcome = await run(() => actions.removeSlice(slice.id));
              if (outcome.ok) onRemoved();
              else setConfirming(false);
            }}
          >
            Delete slice
          </PressButton>
        </div>
      ) : (
        <PressButton variant="ghost" size="sm" className="w-fit text-destructive hover:text-destructive" onClick={() => setConfirming(true)}>
          <Trash2 aria-hidden data-icon="inline-start" />
          Delete slice
        </PressButton>
      )}
      <ActionError error={error} />
    </section>
  );
}
