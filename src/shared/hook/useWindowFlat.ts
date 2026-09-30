import { useEffect, useState } from "react";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";

/** Snaps can resize in several steps; re-check once things settle. */
const SETTLE_MS = 120;

/**
 * True when the window should render without margin, radius and shadow:
 * maximized, fullscreen, or snapped (fills the monitor's work area on
 * either axis).
 *
 * @param tolerance Physical pixels of slack when comparing against the work area.
 */
export function useWindowFlat(tolerance = 16): boolean {
  const [flat, setFlat] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let disposed = false;
    let latest = 0;
    let settleTimer: ReturnType<typeof setTimeout> | undefined;

    const evaluate = async () => {
      const run = ++latest;
      try {
        const [maximized, fullscreen] = await Promise.all([
          win.isMaximized(),
          win.isFullscreen(),
        ]);

        let next = maximized || fullscreen;

        if (!next) {
          const [monitor, pos, size] = await Promise.all([
            currentMonitor(),
            win.outerPosition(),
            win.outerSize(),
          ]);

          if (monitor) {
            const { position: area, size: areaSize } = monitor.workArea;
            const near = (a: number, b: number) => Math.abs(a - b) <= tolerance;

            // Snapped = both edges line up with the work area on at least one axis
            const fillsWidth =
              near(pos.x, area.x) && near(pos.x + size.width, area.x + areaSize.width);
            const fillsHeight =
              near(pos.y, area.y) && near(pos.y + size.height, area.y + areaSize.height);
            next = fillsWidth || fillsHeight;
          }
        }

        if (!disposed && run === latest) setFlat(next);
      } catch (error) {
        console.error("[WindowFrame] Could not read window state", error);
      }

    };

    const schedule = () => {
      void evaluate();
      clearTimeout(settleTimer);
      settleTimer = setTimeout(() => void evaluate(), SETTLE_MS);
    };

    schedule();
    const unlisteners = [win.onResized(schedule), win.onMoved(schedule)];

    return () => {
      disposed = true;
      clearTimeout(settleTimer);
      unlisteners.forEach((p) => p.then((unlisten) => unlisten()));
    };
  }, [tolerance]);

  return flat;
}
