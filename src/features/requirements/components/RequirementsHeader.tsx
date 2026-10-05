import { useState } from "react";
import { CheckCheck, CircleCheck, CircleDashed, Plus } from "lucide-react";
import type { Plan, PlanSummary } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { RequirementActions } from "../hooks/useRequirementActions";
import { orderSlices, plural } from "../lib/order";
import { ActionError } from "./Notices";
import { RequirementDialog } from "./RequirementDialog";

interface RequirementsHeaderProps {
  plan: Plan;
  actions: RequirementActions;
}

/** Counts, Confirm all (which skips unassigned requirements) and Add requirement. */
export function RequirementsHeader({ plan, actions }: RequirementsHeaderProps) {
  const [adding, setAdding] = useState(false);
  const { busy, error, run } = useAction();
  const known = new Set(plan.slices.map((slice) => slice.id));
  const assigned = (sliceId: string | null) => sliceId !== null && known.has(sliceId);
  const confirmed = plan.requirements.filter((requirement) => requirement.state === "confirmed").length;
  const unassigned = plan.requirements.filter((requirement) => !assigned(requirement.sliceId)).length;
  const confirmable = plan.requirements.filter(
    (requirement) => requirement.state === "proposed" && assigned(requirement.sliceId),
  ).length;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <p className="mr-auto text-[13px] text-muted-foreground tabular-nums">
          <span className="font-medium text-foreground">{plural(plan.requirements.length, "requirement")}</span>
          {" · "}
          {confirmed} confirmed
          {unassigned > 0 && ` · ${unassigned} unassigned`}
        </p>
        <PressButton
          variant="secondary"
          size="sm"
          disabled={busy || confirmable === 0}
          title={unassigned > 0 ? "Unassigned requirements stay proposed" : undefined}
          onClick={() => void run(() => actions.confirmAll())}
        >
          <CheckCheck aria-hidden data-icon="inline-start" />
          Confirm all{confirmable > 0 && ` (${confirmable})`}
        </PressButton>
        <PressButton size="sm" onClick={() => setAdding(true)}>
          <Plus aria-hidden data-icon="inline-start" />
          Add requirement
        </PressButton>
      </div>
      {unassigned > 0 && confirmable > 0 && (
        <p className="text-right text-[12px] text-muted-foreground">Confirm all skips the {unassigned} unassigned.</p>
      )}
      <ActionError error={error} />
      <RequirementDialog open={adding} onOpenChange={setAdding} actions={actions} slices={orderSlices(plan.slices)} />
    </div>
  );
}

/** What stands between the plan and "confirmed", from `planSummary`. */
export function SummaryBanner({ plan, summary }: { plan: Plan; summary: PlanSummary | null }) {
  if (!summary) return null;

  if (summary.confirmed) {
    return (
      <div role="status" className="flex items-center gap-2 rounded-2xl bg-muted px-4 py-3 text-[13px]">
        <CircleCheck aria-hidden className="size-4 shrink-0 text-foreground" />
        The plan is confirmed: every requirement has a slice and is confirmed.
      </div>
    );
  }

  const blockers = [
    summary.unassigned.length > 0 && `${summary.unassigned.length} unassigned`,
    summary.unconfirmed.length > 0 && `${summary.unconfirmed.length} unconfirmed`,
    plan.slices.length === 0 && "no slices yet",
    !summary.acyclic && "the dependencies form a cycle",
  ].filter((blocker): blocker is string => Boolean(blocker));

  return (
    <div role="status" className="flex items-start gap-2 rounded-2xl bg-muted px-4 py-3 text-[13px]">
      <CircleDashed aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
      <p>
        <span className="font-medium">Not confirmed yet</span>
        {blockers.length > 0 && <span className="text-muted-foreground">: {blockers.join(", ")}.</span>}
      </p>
    </div>
  );
}
