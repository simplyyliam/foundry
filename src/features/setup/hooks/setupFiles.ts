import { useEffect, useRef } from "react";

/**
 * A frontend-only "setup files were written" signal inside the Setup feature: generating
 * AGENTS.md calls `notifySetupFilesWritten`, and the commit section reads `setupStatus` again.
 * The backend has no event for it. Only reaches views in this window.
 */
type Listener = (projectId: string) => void;

const listeners = new Set<Listener>();

export function notifySetupFilesWritten(projectId: string) {
  listeners.forEach((listener) => listener(projectId));
}

/** Calls `onWritten` whenever `notifySetupFilesWritten(projectId)` runs. */
export function useSetupFilesWritten(projectId: string, onWritten: () => void) {
  const latest = useRef(onWritten);
  useEffect(() => {
    latest.current = onWritten;
  });

  useEffect(() => {
    const listener: Listener = (written) => {
      if (written === projectId) latest.current();
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, [projectId]);
}
