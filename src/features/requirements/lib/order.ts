import type { Plan, PlanRequirement, SliceView } from "@/lib/bindings";

/** Slices by layer, then name: the Blueprint's order. */
export function orderSlices(slices: readonly SliceView[]): SliceView[] {
  return [...slices].sort(
    (a, b) => a.layer - b.layer || a.name.localeCompare(b.name) || a.id.localeCompare(b.id),
  );
}

/** A rule for the whole project. Older plans have no flag, which reads as `false`. */
export const isProjectWide = (requirement: PlanRequirement) => requirement.projectWide === true;

/**
 * Where a requirement stands for confirming: in a known slice, project-wide (the backend
 * counts both as handled), or unassigned. A `sliceId` pointing at a missing slice is unassigned.
 */
export function placement(requirement: PlanRequirement, known: ReadonlySet<string>): "slice" | "project" | "unassigned" {
  if (isProjectWide(requirement)) return "project";
  return requirement.sliceId !== null && known.has(requirement.sliceId) ? "slice" : "unassigned";
}

export type RequirementGroup =
  | { kind: "project"; requirements: PlanRequirement[] }
  | { kind: "unassigned"; requirements: PlanRequirement[] }
  | { kind: "slice"; slice: SliceView; requirements: PlanRequirement[] };

/**
 * Project rules first, then Unassigned (each only when there are any), then every slice in
 * Blueprint order.
 */
export function groupRequirements(plan: Plan): RequirementGroup[] {
  const known = new Set(plan.slices.map((slice) => slice.id));
  const where = (kind: "project" | "unassigned") =>
    plan.requirements.filter((requirement) => placement(requirement, known) === kind);
  const project = where("project");
  const unassigned = where("unassigned");
  return [
    ...(project.length > 0 ? [{ kind: "project" as const, requirements: project }] : []),
    ...(unassigned.length > 0 ? [{ kind: "unassigned" as const, requirements: unassigned }] : []),
    ...orderSlices(plan.slices).map((slice) => ({
      kind: "slice" as const,
      slice,
      requirements: plan.requirements.filter(
        (requirement) => !isProjectWide(requirement) && requirement.sliceId === slice.id,
      ),
    })),
  ];
}

export const plural = (count: number, one: string, many = `${one}s`) =>
  `${count} ${count === 1 ? one : many}`;
