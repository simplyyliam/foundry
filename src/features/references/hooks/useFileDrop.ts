import { useEffect, useRef, useState, type RefObject } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * File drops onto an element. Tauri's drag-drop handling is on (the default; the window
 * config doesn't turn it off), so the webview never sees HTML5 file drops: Tauri reports
 * them with absolute paths and a physical position instead. A drop counts for this element
 * only when the topmost element under the pointer is inside it, so the panel ignores drops
 * aimed at its pop-out (whose overlay covers it) and the other way round.
 *
 * Returns whether files are being dragged over the element.
 */
export function useFileDrop(ref: RefObject<HTMLElement | null>, onDrop: (paths: string[]) => void): boolean {
  const [over, setOver] = useState(false);
  const onDropRef = useRef(onDrop);
  useEffect(() => {
    onDropRef.current = onDrop;
  });

  useEffect(() => {
    const isOver = (position: { x: number; y: number }) => {
      const element = ref.current;
      if (!element) return false;
      const ratio = window.devicePixelRatio || 1;
      const target = document.elementFromPoint(position.x / ratio, position.y / ratio);
      return target !== null && element.contains(target);
    };

    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent(({ payload }) => {
        if (payload.type === "leave") return setOver(false);
        const inside = isOver(payload.position);
        if (payload.type === "drop") {
          setOver(false);
          if (inside && payload.paths.length > 0) onDropRef.current(payload.paths);
        } else {
          setOver(inside);
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      // Outside Tauri (no webview): no file drops.
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [ref]);

  return over;
}
