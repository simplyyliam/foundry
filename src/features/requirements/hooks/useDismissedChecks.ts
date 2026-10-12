import { useCallback, useSyncExternalStore } from "react";
import type { RequirementCheck } from "@/lib/bindings";
import { checkKey } from "@/shared";

/**
 * Suggested checks the user dismissed. Nothing tells the backend: a dismissal only hides the
 * suggestion until the app restarts (module state, so closing the dialog keeps it hidden).
 * Each change replaces the set, so memoized readers see a new value.
 */
let dismissed: ReadonlySet<string> = new Set();
const listeners = new Set<() => void>();

const keyOf = (projectId: string, requirementId: string, check: RequirementCheck) =>
  `${projectId}\u0001${requirementId}\u0001${checkKey(check)}`;

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

function replace(next: ReadonlySet<string>) {
  dismissed = next;
  listeners.forEach((listener) => listener());
}

export function useDismissedChecks(projectId: string, requirementId: string) {
  const current = useSyncExternalStore(subscribe, () => dismissed);

  const isDismissed = useCallback(
    (check: RequirementCheck) => current.has(keyOf(projectId, requirementId, check)),
    [current, projectId, requirementId],
  );
  const dismiss = useCallback(
    (check: RequirementCheck) => replace(new Set(dismissed).add(keyOf(projectId, requirementId, check))),
    [projectId, requirementId],
  );
  return { isDismissed, dismiss };
}

/** Tests only: forget every dismissal. */
export function resetDismissedChecks() {
  replace(new Set());
}
