import type { Plan, PlanDeferred, PlanRequirement, SliceStatus, SliceView } from "@/lib/bindings";

/** Slices by layer, then name (ids break ties so the order never flickers). */
export function orderSlices(slices: readonly SliceView[]): SliceView[] {
  return [...slices].sort(
    (a, b) => a.layer - b.layer || a.name.localeCompare(b.name) || a.id.localeCompare(b.id),
  );
}

/** Requirements per slice id (null = unassigned), each list in the plan's own order. */
export function requirementsBySlice(plan: Plan): Map<string | null, PlanRequirement[]> {
  const known = new Set(plan.slices.map((slice) => slice.id));
  const groups = new Map<string | null, PlanRequirement[]>();
  for (const requirement of plan.requirements) {
    // A dangling slice id can't be drawn in a slice box: treat it as unassigned.
    const key = requirement.sliceId !== null && known.has(requirement.sliceId) ? requirement.sliceId : null;
    const list = groups.get(key);
    if (list) list.push(requirement);
    else groups.set(key, [requirement]);
  }
  return groups;
}

export const plural = (count: number, one: string, many = `${one}s`) =>
  `${count} ${count === 1 ? one : many}`;

export const STATUS_LABEL: Record<SliceStatus, string> = {
  queued: "queued",
  in_progress: "in progress",
  done: "done",
};

/** A "Later" item's anchor name, or a note when the slice it pointed at is gone. */
export const anchorName = (plan: Plan, item: PlanDeferred) =>
  plan.slices.find((slice) => slice.id === item.anchoredTo)?.name ?? "a removed slice";
