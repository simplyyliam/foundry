import { useMemo, useState, type HTMLAttributes, type PointerEvent, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn } from "@/lib/utils";
import { PortalContainerContext } from "@/lib/portal-container";
import { useClickThroughMargin } from "../hook/useClickthrough";
import { useWindowFlat } from "../hook/useWindowFlat";
import { type ResizeDirection, type FrameMetrics, getResizeHandles } from "../utils/geometry";
const DEFAULT_SHADOW =
  "shadow-[0_4px_16px_rgba(0,0,0,0.3),0_0_0_1px_rgba(0,0,0,0.15)]";

export interface WindowFrameProps extends HTMLAttributes<HTMLDivElement> {
  /** Rendered at the top of the card, above the content. */
  titlebar?: ReactNode;
  /** Transparent margin around the card, in px. Must fit the shadow. */
  margin?: number;
  /** Card corner radius, in px. */
  radius?: number;
  /** Thickness of the edge resize strips, in px. */
  resizeEdge?: number;
  /** Size of the corner resize squares, in px. */
  resizeCorner?: number;
  /** Classes for the card (background, padding). */
  cardClassName?: string;
  /** Shadow classes applied while the frame isn't flat. */
  shadowClassName?: string;
  /** Let clicks in the transparent margin pass through to the desktop. */
  clickThrough?: boolean;
}

const startResize = (direction: ResizeDirection) => (e: PointerEvent<HTMLDivElement>) => {
  if (e.button !== 0) return;
  e.preventDefault();
  getCurrentWindow()
    .startResizeDragging(direction)
    .catch((error) => console.error("[WindowFrame] Could not start resize", error));
};

/**
 * Custom window chrome for an undecorated, transparent Tauri window on Windows:
 * rounded card with a CSS shadow, resize handles on the card edges, a
 * click-through margin, and a flat layout when maximized, fullscreen or snapped.
 *
 * `className` and other div props apply to the content area below the titlebar.
 */
export function WindowFrame({
  titlebar,
  margin = 24,
  radius = 15,
  resizeEdge = 8,
  resizeCorner = 16,
  cardClassName = "bg-accent p-1.5",
  shadowClassName = DEFAULT_SHADOW,
  clickThrough = true,
  className,
  children,
  ...props
}: WindowFrameProps) {
  const flat = useWindowFlat();
  // Dialogs and sheets portal here (see @/lib/portal-container); state so they re-render once it exists.
  const [modalLayer, setModalLayer] = useState<HTMLDivElement | null>(null);

  const metrics = useMemo<FrameMetrics>(
    () => ({ margin, radius, edge: resizeEdge, corner: resizeCorner }),
    [margin, radius, resizeEdge, resizeCorner],
  );
  const handles = useMemo(() => getResizeHandles(metrics), [metrics]);

  useClickThroughMargin(clickThrough && !flat, metrics);

  return (
    <div className="relative h-screen" style={{ padding: flat ? 0 : margin }}>
      <div
        className={cn(
          "relative flex h-full flex-col overflow-hidden",
          cardClassName,
          !flat && shadowClassName,
        )}
        style={{ borderRadius: flat ? 0 : radius }}
      >
        <PortalContainerContext.Provider value={modalLayer}>
          {titlebar}
          <div className={cn("min-h-0 flex-1", className)} {...props}>
            {children}
          </div>
        </PortalContainerContext.Provider>

        {/*
          Modal layer: covers the card exactly. `contain: layout` makes it the containing block
          for `fixed` children, so an overlay's `fixed inset-0` fills the card (not the
          transparent margin) and is clipped by the card's rounded corners. It ignores the
          pointer itself; whatever is portaled into it takes clicks again.
        */}
        <div
          ref={setModalLayer}
          data-slot="modal-layer"
          className="pointer-events-none absolute inset-0 z-50 overflow-hidden [contain:layout] *:pointer-events-auto"
        />
      </div>

      {!flat &&
        handles.map(({ direction, cursor, style }) => (
          <div
            key={direction}
            aria-hidden
            onPointerDown={startResize(direction)}
            className="absolute z-50"
            style={{ ...style, cursor }}
          />
        ))}
    </div>
  );
}