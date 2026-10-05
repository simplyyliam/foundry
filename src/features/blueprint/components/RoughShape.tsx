import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import type { RoughPath } from "../lib/rough";

interface RoughShapeProps {
  paths: RoughPath[];
  /** Classes for the outline strokes (e.g. `stroke-slice`). */
  strokeClassName?: string;
  /** Classes for the solid fill (e.g. `fill-slice-fill`). */
  fillClassName?: string;
  strokeWidth?: number;
  dashed?: boolean;
}

/** Renders pre-generated rough.js paths. Purely decorative; text stays in the DOM above it. */
export function RoughShape({ paths, strokeClassName, fillClassName, strokeWidth = 1.4, dashed }: RoughShapeProps) {
  return (
    <>
      {paths.map((path, index) =>
        path.role === "fill" ? (
          <path key={index} d={path.d} className={cn("stroke-none", fillClassName)} />
        ) : (
          <path
            key={index}
            d={path.d}
            fill="none"
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={dashed ? "6 5" : undefined}
            className={strokeClassName}
          />
        ),
      )}
    </>
  );
}

/** An absolutely positioned SVG layer the size of its box, behind the box's text. */
export function RoughBackdrop({ width, height, children }: { width: number; height: number; children: ReactNode }) {
  return (
    <svg
      aria-hidden
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className="pointer-events-none absolute inset-0 overflow-visible"
    >
      {children}
    </svg>
  );
}
