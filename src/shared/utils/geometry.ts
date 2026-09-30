import type { CSSProperties } from "react";

export type ResizeDirection =
  | "North"
  | "South"
  | "East"
  | "West"
  | "NorthEast"
  | "NorthWest"
  | "SouthEast"
  | "SouthWest";

/** All values in CSS (logical) pixels. */
export interface FrameMetrics {
  /** Transparent space between the window edge and the card (room for the shadow). */
  margin: number;
  /** Card corner radius. */
  radius: number;
  /** Thickness of the edge resize strips. */
  edge: number;
  /** Side length of the corner resize squares. */
  corner: number;
}

export interface ResizeHandle {
  direction: ResizeDirection;
  cursor: CSSProperties["cursor"];
  style: CSSProperties;
}

/**
 * Resize handles centered on the card's edges and corners.
 * Edge strips stop where the corner squares begin so they never overlap.
 */
export function getResizeHandles({ margin, edge, corner }: FrameMetrics): ResizeHandle[] {
  const e = margin - edge / 2;
  const c = margin - corner / 2;
  const span = margin + corner / 2;
  const sq = { width: corner, height: corner };

  return [
    { direction: "North", cursor: "ns-resize", style: { top: e, left: span, right: span, height: edge } },
    { direction: "South", cursor: "ns-resize", style: { bottom: e, left: span, right: span, height: edge } },
    { direction: "West", cursor: "ew-resize", style: { left: e, top: span, bottom: span, width: edge } },
    { direction: "East", cursor: "ew-resize", style: { right: e, top: span, bottom: span, width: edge } },
    { direction: "NorthWest", cursor: "nwse-resize", style: { top: c, left: c, ...sq } },
    { direction: "NorthEast", cursor: "nesw-resize", style: { top: c, right: c, ...sq } },
    { direction: "SouthWest", cursor: "nesw-resize", style: { bottom: c, left: c, ...sq } },
    { direction: "SouthEast", cursor: "nwse-resize", style: { bottom: c, right: c, ...sq } },
  ];
}

/**
 * Whether a point (relative to the window's top-left, in CSS px) should be
 * interactive: the rounded card grown by the handles' overhang, plus the
 * corner resize squares.
 */
export function isInsideHitArea(
  x: number,
  y: number,
  width: number,
  height: number,
  { margin, radius, corner }: FrameMetrics,
): boolean {
  const slack = corner / 2;
  const left = margin - slack;
  const top = margin - slack;
  const right = width - left;
  const bottom = height - top;

  if (x < left || x > right || y < top || y > bottom) return false;

  // Corner resize squares are fully interactive.
  const nearX = x <= left + corner || x >= right - corner;
  const nearY = y <= top + corner || y >= bottom - corner;
  if (nearX && nearY) return true;

  // Rounded-rect test for everything else.
  const r = radius + slack;
  const cx = Math.min(Math.max(x, left + r), right - r);
  const cy = Math.min(Math.max(y, top + r), bottom - r);
  const dx = x - cx;
  const dy = y - cy;
  return dx * dx + dy * dy <= r * r;
}