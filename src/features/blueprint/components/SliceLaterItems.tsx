import { useId, useState } from "react";
import { Plus } from "lucide-react";
import { Input } from "@/components/ui/input";
import type { Plan, PlanDeferred, SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { DeferredMove, DeferredRemove, SliceSelect } from "./DeferredControls";
import { ActionError } from "./Notices";

interface SliceLaterItemsProps {
  plan: Plan;
  slice: SliceView;
  actions: PlanActions;
}

/**
 * The "Later" items anchored to this slice, each with Move and Remove, and "Add later item".
 * Sits right above Delete slice, which the backend refuses while any are anchored here.
 */
export function SliceLaterItems({ plan, slice, actions }: SliceLaterItemsProps) {
  const items = plan.deferred.filter((item) => item.anchoredTo === slice.id);
  return (
    <div className="flex flex-col gap-2">
      {items.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">None anchored here.</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {items.map((item) => (
            <LaterItem key={item.id} plan={plan} item={item} actions={actions} />
          ))}
        </ul>
      )}
      <AddLaterItem plan={plan} slice={slice} actions={actions} />
    </div>
  );
}

function LaterItem({ plan, item, actions }: { plan: Plan; item: PlanDeferred; actions: PlanActions }) {
  const [mode, setMode] = useState<"idle" | "move" | "remove">("idle");
  return (
    <li className="flex flex-col gap-1.5 rounded-xl bg-muted/60 px-3 py-2">
      <div className="flex items-start gap-1">
        <p className="min-w-0 flex-1 py-0.5 text-[13px] leading-snug">{item.label}</p>
        <PressButton
          variant="ghost"
          size="xs"
          className="-my-0.5 shrink-0"
          aria-expanded={mode === "move"}
          onClick={() => setMode(mode === "move" ? "idle" : "move")}
        >
          Move
        </PressButton>
        <PressButton
          variant="ghost"
          size="xs"
          className="-my-0.5 shrink-0 text-destructive hover:text-destructive"
          aria-expanded={mode === "remove"}
          onClick={() => setMode(mode === "remove" ? "idle" : "remove")}
        >
          Remove
        </PressButton>
      </div>
      {mode === "move" && <DeferredMove plan={plan} item={item} actions={actions} />}
      {mode === "remove" && <DeferredRemove item={item} actions={actions} onCancel={() => setMode("idle")} />}
    </li>
  );
}

/** A label, anchored to this slice unless another is picked. */
function AddLaterItem({ plan, slice, actions }: SliceLaterItemsProps) {
  const id = useId();
  const [label, setLabel] = useState("");
  const [anchor, setAnchor] = useState(slice.id);
  const { busy, error, run } = useAction();
  const empty = label.trim() === "";

  return (
    <form
      className="flex flex-col gap-1.5"
      onSubmit={async (event) => {
        event.preventDefault();
        if (empty || busy) return;
        if ((await run(() => actions.addDeferred(label.trim(), anchor))).ok) {
          setLabel("");
          setAnchor(slice.id);
        }
      }}
    >
      <Input
        id={id}
        value={label}
        onChange={(event) => setLabel(event.target.value)}
        placeholder="Add a later item"
        aria-label="Later item label"
        className="h-8 text-[13px]"
      />
      <div className="flex items-center gap-1.5">
        <span className="shrink-0 text-[12.5px] text-muted-foreground">after</span>
        <SliceSelect plan={plan} value={anchor} onChange={setAnchor} disabled={busy} label="Anchored to slice" />
        <PressButton type="submit" size="sm" variant="secondary" disabled={empty || busy} className="shrink-0">
          <Plus aria-hidden data-icon="inline-start" />
          Add later item
        </PressButton>
      </div>
      <ActionError error={error} />
    </form>
  );
}
