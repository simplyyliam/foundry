import type { Requirement, Slice } from "@/lib/bindings";

/**
 * Slices in dependency order (Kahn's algorithm): a slice comes after everything it depends
 * on; ties keep the roadmap's own order. Slices caught in a cycle, or depending on names that
 * don't exist, are appended in their original order rather than dropped.
 */
export function buildOrder(slices: readonly Slice[]): Slice[] {
  const names = new Set(slices.map((slice) => slice.name));
  const placed = new Set<string>();
  const ordered: Slice[] = [];
  let progressed = true;
  while (progressed) {
    progressed = false;
    for (const slice of slices) {
      if (placed.has(slice.name)) continue;
      const ready = slice.dependsOn.every((dep) => !names.has(dep) || placed.has(dep));
      if (ready) {
        placed.add(slice.name);
        ordered.push(slice);
        progressed = true;
      }
    }
  }
  return [...ordered, ...slices.filter((slice) => !placed.has(slice.name))];
}

/** Requirements per slice name. */
export function countBySlice(requirements: readonly Requirement[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const requirement of requirements) {
    if (requirement.sliceName) counts.set(requirement.sliceName, (counts.get(requirement.sliceName) ?? 0) + 1);
  }
  return counts;
}
