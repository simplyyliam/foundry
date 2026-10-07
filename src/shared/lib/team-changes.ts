import { useEffect, useRef } from "react";

/**
 * A frontend-only "the team changed" signal. The backend has no team-changed event, so
 * whatever changes a team (adding an agent) calls `notifyTeamChanged`, and every mounted
 * view that shows that project's team (Setup, Teams, Agent Chats) reads `getTeam` again.
 * Only reaches views in this window.
 */
type Listener = (projectId: string) => void;

const listeners = new Set<Listener>();

export function notifyTeamChanged(projectId: string) {
  listeners.forEach((listener) => listener(projectId));
}

/** Calls `onChange` whenever `notifyTeamChanged(projectId)` runs. */
export function useTeamChanged(projectId: string, onChange: () => void) {
  const latest = useRef(onChange);
  useEffect(() => {
    latest.current = onChange;
  });

  useEffect(() => {
    const listener: Listener = (changed) => {
      if (changed === projectId) latest.current();
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, [projectId]);
}
