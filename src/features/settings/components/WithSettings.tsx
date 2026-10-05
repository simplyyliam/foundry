import { LoaderCircle } from "lucide-react";
import { useEffect, type ReactNode } from "react";
import type { Settings } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { reloadSettings, useSettingsState } from "../hooks/settingsStore";

/**
 * Renders a settings-backed section once the settings are loaded. Re-reads them when the
 * section mounts (queued behind any save still running), so a change made elsewhere shows.
 */
export function WithSettings({ children }: { children: (settings: Settings) => ReactNode }) {
  const state = useSettingsState();

  useEffect(() => {
    void reloadSettings();
  }, []);

  if (state.status === "loading") {
    return (
      <div className="flex justify-center py-10">
        <LoaderCircle aria-label="Loading settings" className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (state.status === "error") {
    return (
      <div className="flex flex-col items-start gap-3 py-4">
        <p role="alert" className="text-[12px] text-destructive">
          {state.message}
        </p>
        <PressButton size="sm" variant="outline" className="rounded-md" onClick={() => void reloadSettings()}>
          Try again
        </PressButton>
      </div>
    );
  }
  return <>{children(state.settings)}</>;
}
