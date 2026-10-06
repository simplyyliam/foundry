import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type HandbookRole } from "@/lib/bindings";
import { useSettingsDialog } from "@/shared";
import { call } from "./call";

export interface HandbookRoles {
  /** Null until the first answer. */
  roles: HandbookRole[] | null;
  /** The backend's message when the list couldn't be read. */
  error: string | null;
  /** Lists again (the handbook can change in Settings while this page is open). */
  reload: () => void;
}

/** The Role Handbook, read-only: the roles a card can be made from, and their titles. */
export function useHandbookRoles(): HandbookRoles {
  const [roles, setRoles] = useState<HandbookRole[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latestFetch = useRef(0);

  const reload = useCallback(() => {
    const request = ++latestFetch.current;
    void call(() => commands.roleHandbookList()).then((outcome) => {
      if (request !== latestFetch.current) return;
      if (outcome.ok) setRoles(outcome.data);
      setError(outcome.ok ? null : outcome.error.message);
    });
  }, []);

  useEffect(() => {
    reload();
    return () => {
      latestFetch.current++;
    };
  }, [reload]);

  // The handbook is edited in Settings, and there's no event for it: list again when it closes.
  const { open: settingsOpen } = useSettingsDialog();
  const wasOpen = useRef(settingsOpen);
  useEffect(() => {
    if (wasOpen.current && !settingsOpen) reload();
    wasOpen.current = settingsOpen;
  }, [settingsOpen, reload]);

  return { roles, error, reload };
}
