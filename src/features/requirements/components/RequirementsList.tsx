import { cn } from "@/lib/utils";
import type { Plan } from "@/lib/bindings";
import type { RequirementActions } from "../hooks/useRequirementActions";
import { groupRequirements, orderSlices, plural } from "../lib/order";
import { RequirementRow } from "./RequirementRow";

const STATUS_LABEL = { queued: "queued", in_progress: "in progress", done: "done" } as const;

/** Requirements grouped by slice: Unassigned first, then slices in Blueprint order. */
export function RequirementsList({ plan, actions }: { plan: Plan; actions: RequirementActions }) {
  const slices = orderSlices(plan.slices);
  const groups = groupRequirements(plan);

  if (groups.length === 0) {
    return (
      <p className="px-2 py-10 text-center text-sm text-muted-foreground">
        No requirements yet. Add one, or generate a roadmap from the Roadmap panel.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      {groups.map(({ slice, requirements }) => (
        <section key={slice?.id ?? "unassigned"} aria-label={slice?.name ?? "Unassigned"} className="flex flex-col gap-1">
          <header
            className={cn(
              "flex items-baseline gap-2 px-3 pb-1",
              !slice && "rounded-xl border border-dashed border-muted-foreground/40 py-2",
            )}
          >
            <h2 className="text-[13px] font-semibold">{slice?.name ?? "Unassigned"}</h2>
            <span className="text-[12px] text-muted-foreground tabular-nums">{plural(requirements.length, "requirement")}</span>
            {slice && <span className="ml-auto text-[12px] text-muted-foreground">{STATUS_LABEL[slice.status]}</span>}
          </header>
          {requirements.length === 0 ? (
            <p className="px-3 py-1 text-[13px] text-muted-foreground">Nothing assigned yet.</p>
          ) : (
            <ul className="flex flex-col">
              {requirements.map((requirement) => (
                <RequirementRow key={requirement.id} requirement={requirement} slices={slices} actions={actions} />
              ))}
            </ul>
          )}
        </section>
      ))}
    </div>
  );
}
