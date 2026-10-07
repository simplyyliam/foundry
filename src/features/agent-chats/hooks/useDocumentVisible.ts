import { useSyncExternalStore } from "react";

function subscribe(listener: () => void) {
  document.addEventListener("visibilitychange", listener);
  return () => document.removeEventListener("visibilitychange", listener);
}

/** False while the window is minimized or hidden: events on screen then aren't seen yet. */
export function useDocumentVisible(): boolean {
  return useSyncExternalStore(subscribe, () => document.visibilityState === "visible");
}
