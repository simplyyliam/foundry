import { commands } from "@/lib/bindings";

/** The window card's padding in px. Matches `--shell-padding` in index.css. */
export const SHELL_PADDING = 6;
/** Smallest inner radius worth following the window's; below it the defaults stay. */
const MIN_INSET_RADIUS = 4;

/**
 * Reads the OS window corner radius once and exposes it to CSS: `--window-radius` and
 * `data-window-radius="custom"` on `<html>`, plus `data-inset-radius="window"` when the
 * sidebar and inset cards can follow it (see index.css). Sets nothing when the radius
 * is unknown or the call fails, so the default radii apply.
 */
export async function loadWindowRadius(root: HTMLElement = document.documentElement): Promise<void> {
  try {
    const res = await commands.getWindowRadius();
    if (res.status !== "ok" || typeof res.data.radius !== "number") return;
    const { radius } = res.data;

    root.style.setProperty("--window-radius", `${radius}px`);
    root.dataset.windowRadius = "custom";
    if (radius - SHELL_PADDING >= MIN_INSET_RADIUS) root.dataset.insetRadius = "window";
  } catch (error) {
    console.error("[window-radius] Could not read the window radius", error);
  }
}
