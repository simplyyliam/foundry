import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

/**
 * Whether the Settings dialog is open, and which section it shows. Lives in `@/shared` so
 * the sidebar's gear (one widget) can open the dialog (another widget) without importing it.
 */
interface SettingsDialogState {
  open: boolean;
  /** Section id to show; null keeps whatever was shown last. */
  section: string | null;
  openSettings: (section?: string) => void;
  setOpen: (open: boolean) => void;
}

const SettingsDialogContext = createContext<SettingsDialogState | null>(null);

export function SettingsDialogProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [section, setSection] = useState<string | null>(null);

  const openSettings = useCallback((next?: string) => {
    setSection(next ?? null);
    setOpen(true);
  }, []);

  const value = useMemo(() => ({ open, section, openSettings, setOpen }), [open, section, openSettings]);
  return <SettingsDialogContext.Provider value={value}>{children}</SettingsDialogContext.Provider>;
}

export function useSettingsDialog(): SettingsDialogState {
  const context = useContext(SettingsDialogContext);
  if (!context) throw new Error("useSettingsDialog must be used inside SettingsDialogProvider");
  return context;
}
