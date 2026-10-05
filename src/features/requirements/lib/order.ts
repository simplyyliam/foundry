import type { Plan, PlanRequirement, SliceView } from "@/lib/bindings";

/** Slices by layer, then name: the Blueprint's order. */
export function orderSlices(slices: readonly SliceView[]): SliceView[] {
  return [...slices].sort(
    (a, b) => a.layer - b.layer || a.name.localeCompare(b.name) || a.id.localeCompare(b.id),
  );
}

export interface RequirementGroup {
  /** Null for the unassigned group. */
  slice: SliceView | null;
  requirements: PlanRequirement[];
}

/** Unassigned first (only when there are any), then every slice in Blueprint order. */
export function groupRequirements(plan: Plan): RequirementGroup[] {
  const known = new Set(plan.slices.map((slice) => slice.id));
  const isUnassigned = (requirement: PlanRequirement) =>
    requirement.sliceId === null || !known.has(requirement.sliceId);
  const unassigned = plan.requirements.filter(isUnassigned);
  return [
    ...(unassigned.length > 0 ? [{ slice: null, requirements: unassigned }] : []),
    ...orderSlices(plan.slices).map((slice) => ({
      slice,
      requirements: plan.requirements.filter((requirement) => requirement.sliceId === slice.id),
    })),
  ];
}

export const plural = (count: number, one: string, many = `${one}s`) =>
  `${count} ${count === 1 ? one : many}`;
