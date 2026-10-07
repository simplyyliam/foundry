import { useState } from "react";
import { Trash2 } from "lucide-react";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { Plan, PlanDeferred } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { PlanActions } from "../hooks/usePlanActions";
import { anchorName } from "../lib/order";
import { DeferredMove, DeferredRemove } from "./DeferredControls";

interface DeferredSheetProps {
  plan: Plan;
  /** The "Later" item to show, or null when closed. */
  deferredId: string | null;
  actions: PlanActions;
  /** A move gives the item a new id; the sheet follows it. */
  onMoved: (newId: string) => void;
  onClose: () => void;
}

/** A "Later" item: where it sits, moving it to another slice, and removing it. */
export function DeferredSheet({ plan, deferredId, actions, onMoved, onClose }: DeferredSheetProps) {
  // Keep showing the last item while the sheet animates out.
  const [shown, setShown] = useState<PlanDeferred | null>(null);
  const current = plan.deferred.find((item) => item.id === deferredId) ?? null;
  if (current && current !== shown) setShown(current);
  const item = current ?? shown;

  return (
    <Sheet open={current !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full gap-0 sm:max-w-md">
        {item && <DeferredDetails key={item.id} plan={plan} item={item} actions={actions} onMoved={onMoved} onRemoved={onClose} />}
      </SheetContent>
    </Sheet>
  );
}

interface DeferredDetailsProps {
  plan: Plan;
  item: PlanDeferred;
  actions: PlanActions;
  onMoved: (newId: string) => void;
  onRemoved: () => void;
}

function DeferredDetails({ plan, item, actions, onMoved, onRemoved }: DeferredDetailsProps) {
  const [confirming, setConfirming] = useState(false);

  return (
    <>
      <SheetHeader className="gap-1 pr-14">
        <p className="text-[12px] font-medium tracking-wide text-muted-foreground uppercase">Later</p>
        <SheetTitle className="text-lg leading-snug text-pretty">{item.label}</SheetTitle>
        <SheetDescription className="text-[13px]">after {anchorName(plan, item)}</SheetDescription>
      </SheetHeader>
      <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-6 pb-6">
        <section className="flex flex-col gap-2">
          <h3 className="text-[12px] font-medium tracking-wide text-muted-foreground uppercase">Move to slice</h3>
          <p className="text-[12.5px] leading-snug text-muted-foreground">
            The item is added at the new slice first, then removed here.
          </p>
          <DeferredMove plan={plan} item={item} actions={actions} onMoved={onMoved} />
        </section>
        <section className="mt-2 flex flex-col gap-2 border-t border-border/60 pt-4">
          {confirming ? (
            <DeferredRemove item={item} actions={actions} onCancel={() => setConfirming(false)} onRemoved={onRemoved} />
          ) : (
            <PressButton
              variant="ghost"
              size="sm"
              className="w-fit text-destructive hover:text-destructive"
              onClick={() => setConfirming(true)}
            >
              <Trash2 aria-hidden data-icon="inline-start" />
              Remove later item
            </PressButton>
          )}
        </section>
      </div>
    </>
  );
}
