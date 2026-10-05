import { useEffect, useSyncExternalStore } from "react";
import type { Theme } from "@/lib/bindings";
import { useSettingsState } from "./settingsStore";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function subscribeToOsTheme(listener: () => void) {
  const media = window.matchMedia(DARK_QUERY);
  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}

/** Live OS dark-mode preference. */
function useOsPrefersDark(): boolean {
  return useSyncExternalStore(subscribeToOsTheme, () => window.matchMedia(DARK_QUERY).matches);
}

/**
 * Applies the saved theme and reduced-motion override to the whole app. Mount once, somewhere
 * always present (`App.tsx`), so both apply at startup even when the Settings dialog is closed.
 *
 * Theme: the `.dark` class on `<html>`, the same switch the `dark:` variant and the dark tokens
 * in `index.css` key off. Until settings load it follows the OS ("system").
 *
 * Reduced motion: `data-reduce-motion` on `<html>` feeds Tailwind's `motion-reduce:` variant
 * (see `index.css`); the returned flag feeds motion's `MotionConfig`.
 */
export function useAppearance(): { reduceMotion: boolean } {
  const state = useSettingsState();
  const osDark = useOsPrefersDark();
  const settings = state.status === "ready" ? state.settings : null;
  const theme: Theme = settings?.theme ?? "system";
  const reduceMotion = settings?.reducedMotionOverride ?? false;
  const dark = theme === "dark" || (theme === "system" && osDark);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", dark);
    // Native scrollbars and form controls follow too.
    root.style.colorScheme = dark ? "dark" : "light";
  }, [dark]);

  useEffect(() => {
    document.documentElement.toggleAttribute("data-reduce-motion", reduceMotion);
  }, [reduceMotion]);

  return { reduceMotion };
}
