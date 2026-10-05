import { useSyncExternalStore } from "react";

/**
 * Which project view the panel shows on project routes. One choice for the whole session,
 * across pages and projects; it isn't saved, so every launch starts on Roadmap.
 */
export type ProjectViewId = "roadmap" | "references";

export const PROJECT_VIEW_IDS: readonly ProjectViewId[] = ["roadmap", "references"];

let current: ProjectViewId = "roadmap";
const listeners = new Set<() => void>();

export function setProjectView(view: ProjectViewId) {
  if (view === current) return;
  current = view;
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useProjectView(): ProjectViewId {
  return useSyncExternalStore(subscribe, () => current);
}
