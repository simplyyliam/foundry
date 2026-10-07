import { useId, useState } from "react";
import { Trash2 } from "lucide-react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { AppError, Plan, PlanDeferred } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { orderSlices } from "../lib/order";
import { ActionError } from "./Notices";

const REMOVE_QUESTION = "Remove this later item? Agents no longer see it as a future direction.";
const HALF_MOVED = "Added at the new slice, but the old item couldn't be removed.";

interface SliceSelectProps {
  plan: Plan;
  value: string | null;
  onChange: (sliceId: string) => void;
  /** A slice that can't be picked (the item's current anchor). */
  current?: string;
  disabled?: boolean;
  id?: string;
  label: string;
  className?: string;
}

/** The plan's slices in Blueprint order (layer, then name). */
export function SliceSelect({ plan, value, onChange, current, disabled, id, label, className }: SliceSelectProps) {
  const slices = orderSlices(plan.slices);
  const picked = slices.find((slice) => slice.id === value) ?? null;
  return (
    <Select
      value={picked?.id ?? null}
      items={slices.map((slice) => ({ value: slice.id, label: slice.name }))}
      disabled={disabled}
      onValueChange={(next) => {
        if (typeof next === "string") onChange(next);
      }}
    >
      <SelectTrigger id={id} size="sm" aria-label={label} className={className ?? "w-full min-w-0 text-[13px]"}>
        <SelectValue>
          {() => (picked ? <span className="truncate">{picked.name}</span> : <span className="text-muted-foreground">Pick a slice</span>)}
        </SelectValue>
      </SelectTrigger>
      <SelectContent alignItemWithTrigger={false} className="min-w-56">
        {slices.map((slice) => (
          <SelectItem key={slice.id} value={slice.id} disabled={slice.id === current} className="text-[13px]">
            <span className="min-w-0 truncate">{slice.name}</span>
            {slice.id === current && <span className="ml-auto text-[11px] text-muted-foreground">current</span>}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

interface DeferredMoveProps {
  plan: Plan;
  item: PlanDeferred;
  actions: PlanActions;
  /** After a complete move: the item now lives on under `newId`. */
  onMoved?: (newId: string) => void;
}

/**
 * Move to another slice: `addDeferred` there, then `removeDeferred` of this item (there's no
 * move command). If the add fails nothing changed; if the remove fails both copies stay, and
 * this offers to remove the old one.
 */
export function DeferredMove({ plan, item, actions, onMoved }: DeferredMoveProps) {
  const id = useId();
  const [target, setTarget] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  /** The add worked and the remove didn't: the new item's id; the old one (this) is still here. */
  const [halfMoved, setHalfMoved] = useState<string | null>(null);
  const cleanup = useAction();

  if (halfMoved) {
    return (
      <div className="flex flex-col gap-2">
        <div role="alert" className="flex flex-col gap-2 rounded-xl bg-destructive/8 px-3 py-2 text-[13px]">
          <p className="font-medium">{HALF_MOVED}</p>
          {error && <p className="text-muted-foreground">{error.message}</p>}
          <PressButton
            size="sm"
            variant="destructive"
            className="w-fit"
            disabled={cleanup.busy}
            onClick={async () => {
              if ((await cleanup.run(() => actions.removeDeferred(item.id))).ok) onMoved?.(halfMoved);
            }}
          >
            <Trash2 aria-hidden data-icon="inline-start" />
            Remove the old item
          </PressButton>
        </div>
        <ActionError error={cleanup.error} />
      </div>
    );
  }

  return (
    <form
      className="flex flex-col gap-2"
      onSubmit={async (event) => {
        event.preventDefault();
        if (!target || busy) return;
        setBusy(true);
        setError(null);
        const outcome = await actions.moveDeferred(item.id, item.label, target);
        setBusy(false);
        if (outcome.ok) onMoved?.(outcome.newId);
        else {
          setError(outcome.error);
          if (outcome.stage === "remove") setHalfMoved(outcome.newId);
        }
      }}
    >
      <div className="flex items-center gap-1.5">
        <SliceSelect
          id={id}
          plan={plan}
          value={target}
          onChange={setTarget}
          current={item.anchoredTo}
          disabled={busy}
          label={`Move “${item.label}” to slice`}
        />
        <PressButton type="submit" size="sm" variant="secondary" disabled={!target || busy}>
          Move
        </PressButton>
      </div>
      <ActionError error={error} />
    </form>
  );
}

/** Remove, after "Remove this later item? …". */
export function DeferredRemove({
  item,
  actions,
  onCancel,
  onRemoved,
}: {
  item: PlanDeferred;
  actions: PlanActions;
  onCancel: () => void;
  onRemoved?: () => void;
}) {
  const { busy, error, run } = useAction();
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-2 rounded-xl bg-muted/60 px-3 py-2">
        <p className="text-[13px] leading-snug">{REMOVE_QUESTION}</p>
        <div className="flex justify-end gap-1.5">
          <PressButton variant="ghost" size="sm" onClick={onCancel}>
            Cancel
          </PressButton>
          <PressButton
            variant="destructive"
            size="sm"
            disabled={busy}
            onClick={async () => {
              if ((await run(() => actions.removeDeferred(item.id))).ok) onRemoved?.();
            }}
          >
            Remove
          </PressButton>
        </div>
      </div>
      <ActionError error={error} />
    </div>
  );
}
