import { Plus, X } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Plan, SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { orderSlices } from "../lib/order";
import { ActionError } from "./Notices";
import { StatusChip } from "./StatusChip";

interface SliceDependenciesProps {
  plan: Plan;
  slice: SliceView;
  actions: PlanActions;
}

/** "Depends on" with remove and add: the keyboard path for what the canvas does by dragging. */
export function SliceDependencies({ plan, slice, actions }: SliceDependenciesProps) {
  const { busy, error, run } = useAction();
  const byId = new Map(plan.slices.map((candidate) => [candidate.id, candidate]));
  const candidates = orderSlices(plan.slices).filter(
    (candidate) => candidate.id !== slice.id && !slice.dependsOn.includes(candidate.id),
  );
  const usedBy = orderSlices(plan.slices.filter((candidate) => candidate.dependsOn.includes(slice.id)));

  const save = (dependsOn: string[]) => void run(() => actions.setDependencies(slice.id, dependsOn));

  return (
    <div className="flex flex-col gap-2">
      {slice.dependsOn.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">Nothing: it can start any time.</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {slice.dependsOn.map((id) => {
            const dependency = byId.get(id);
            return (
              <li key={id} className="flex items-center gap-2 rounded-xl bg-muted/60 py-1 pr-1 pl-3">
                <span className="min-w-0 flex-1 truncate text-[13px]">{dependency?.name ?? "Removed slice"}</span>
                {dependency && <StatusChip status={dependency.status} />}
                <RemoveButton
                  label={`Remove dependency on ${dependency?.name ?? "removed slice"}`}
                  disabled={busy}
                  onClick={() => save(slice.dependsOn.filter((other) => other !== id))}
                />
              </li>
            );
          })}
        </ul>
      )}
      {candidates.length > 0 && (
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <PressButton variant="ghost" size="xs" className="w-fit text-muted-foreground" disabled={busy}>
                <Plus aria-hidden data-icon="inline-start" />
                Add dependency
              </PressButton>
            }
          />
          <DropdownMenuContent className="w-64">
            {candidates.map((candidate) => (
              <DropdownMenuItem key={candidate.id} onClick={() => save([...slice.dependsOn, candidate.id])}>
                <span className="min-w-0 flex-1 truncate">{candidate.name}</span>
                <StatusChip status={candidate.status} />
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
      <ActionError error={error} />
      {usedBy.length > 0 && (
        <p className="text-[12px] text-muted-foreground">
          Needed by {usedBy.map((other) => other.name).join(", ")}
        </p>
      )}
    </div>
  );
}

function RemoveButton({ label, disabled, onClick }: { label: string; disabled: boolean; onClick: () => void }) {
  return (
    <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label={label} title={label} disabled={disabled} onClick={onClick} className="rounded-full">
      <X aria-hidden />
    </PressButton>
  );
}
