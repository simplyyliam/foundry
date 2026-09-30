import { useEffect, useRef } from "react";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { type FrameMetrics, isInsideHitArea } from "../utils/geometry";

/**
 * Makes the transparent margin around the card click-through.
 *
 * A click-through window receives no mouse events, so the cursor is polled
 * globally and click-through is toggled based on whether it's over the card.
 *
 * @param active   Disable while the frame is flat (no margin to click through).
 * @param metrics  Frame geometry used for the hit test.
 * @param pollMs   Poll interval. Lower is tighter at the boundary but costs an IPC call per tick.
 */
export function useClickThroughMargin(active: boolean, metrics: FrameMetrics, pollMs = 30) {
  const activeRef = useRef(active);
  const metricsRef = useRef(metrics);

  useEffect(() => {
    activeRef.current = active;
    metricsRef.current = metrics;
  });

  useEffect(() => {
    const win = getCurrentWindow();
    let disposed = false;
    let ignoring = false;
    let pressed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    // Cached window geometry (physical px) so each tick costs a single IPC call.
    let origin = { x: 0, y: 0 };
    let size = { width: 0, height: 0 };
    let scale = window.devicePixelRatio;

    const setIgnore = (next: boolean) => {
      if (next === ignoring) return;
      ignoring = next;
      win.setIgnoreCursorEvents(next).catch((error) => {
        ignoring = !next;
        console.error("[WindowFrame] Could not toggle click-through", error);
      });
    };

    Promise.all([win.outerPosition(), win.outerSize(), win.scaleFactor()])
      .then(([p, s, f]) => {
        origin = p;
        size = s;
        scale = f;
      })
      .catch((error) => console.error("[WindowFrame] Could not read window geometry", error));

    const unlisteners = [
      win.onMoved(({ payload }) => (origin = payload)),
      win.onResized(({ payload }) => (size = payload)),
      win.onScaleChanged(({ payload }) => {
        scale = payload.scaleFactor;
        size = payload.size;
      }),
    ];

    // Never flip to click-through mid-drag, or titlebar drags and resizes break.
    // OS-driven drags can swallow pointerup, so also release on the next
    // button-less move and on blur.
    const onDown = () => (pressed = true);
    const onRelease = () => (pressed = false);
    const onMove = (e: PointerEvent) => {
      if (e.buttons === 0) pressed = false;
    };

    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("pointerup", onRelease, true);
    window.addEventListener("pointercancel", onRelease, true);
    window.addEventListener("pointermove", onMove, true);
    window.addEventListener("blur", onRelease);

    // Self-scheduling loop: a slow IPC call never overlaps the next tick.
    const tick = async () => {
      try {
        if (!activeRef.current) {
          setIgnore(false);
        } else if (!pressed) {
          const cursor = await cursorPosition();
          const x = (cursor.x - origin.x) / scale;
          const y = (cursor.y - origin.y) / scale;
          const inside = isInsideHitArea(
            x,
            y,
            size.width / scale,
            size.height / scale,
            metricsRef.current,
          );
          setIgnore(!inside);
        }
      } catch (error) {
        console.error("[WindowFrame] Click-through tick failed", error);
      } finally {
        if (!disposed) timer = setTimeout(tick, pollMs);
      }
    };
    void tick();

    return () => {
      disposed = true;
      clearTimeout(timer);
      unlisteners.forEach((p) => p.then((unlisten) => unlisten()));
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("pointerup", onRelease, true);
      window.removeEventListener("pointercancel", onRelease, true);
      window.removeEventListener("pointermove", onMove, true);
      window.removeEventListener("blur", onRelease);
      win.setIgnoreCursorEvents(false).catch(() => {});
    };
  }, [pollMs]);
}