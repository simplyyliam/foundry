import { memo, useMemo } from "react";
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type Edge, type EdgeProps } from "@xyflow/react";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";
import { usePressScale } from "@/shared";
import { roughLines, roughPath } from "../lib/rough";
import { RoughShape } from "./RoughShape";

/** `source` is the dependency, `target` the slice that depends on it. */
export type DependencyEdgeType = Edge<{ onRemove: (edgeId: string) => void }, "dependency">;

/**
 * A hand-drawn dependency arrow. The visible line is rough.js (seeded by the edge id, so it
 * holds still); React Flow's own path is kept invisible for hit-testing and selection.
 */
export const DependencyEdge = memo(function DependencyEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  selected,
  data,
}: EdgeProps<DependencyEdgeType>) {
  const [path, labelX, labelY] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
  const line = useMemo(() => roughPath(id, path, { roughness: 0.7, bowing: 0.6 }), [id, path]);
  const head = useMemo(
    () =>
      roughLines(`${id}:head`, [
        [targetX - 9, targetY - 5],
        [targetX - 1, targetY],
        [targetX - 9, targetY + 5],
      ], { roughness: 0.5 }),
    [id, targetX, targetY],
  );
  const strokeClassName = cn(
    "transition-[stroke] duration-150",
    selected ? "stroke-slice-foreground" : "stroke-muted-foreground/80 group-hover/edge:stroke-foreground/70",
  );

  return (
    <g className="group/edge">
      <BaseEdge id={id} path={path} interactionWidth={18} style={{ stroke: "transparent" }} />
      <g className="pointer-events-none">
        <RoughShape paths={line} strokeClassName={strokeClassName} strokeWidth={selected ? 2 : 1.4} />
        <RoughShape paths={head} strokeClassName={strokeClassName} strokeWidth={selected ? 2 : 1.4} />
      </g>
      {selected && data && (
        <EdgeLabelRenderer>
          {/* Positioned by the wrapper: the press-scale owns the button's own transform. */}
          <div
            className="nodrag nopan pointer-events-auto absolute"
            style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }}
          >
            <RemoveButton onClick={() => data.onRemove(id)} />
          </div>
        </EdgeLabelRenderer>
      )}
    </g>
  );
});

/** Its own component so the press-scale attaches when it mounts (it only shows while selected). */
function RemoveButton({ onClick }: { onClick: () => void }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.92 });
  return (
    <button
      ref={ref}
      type="button"
      aria-label="Remove this dependency"
      title="Remove dependency (Delete)"
      onClick={onClick}
      className="flex size-6 items-center justify-center rounded-full bg-popover text-muted-foreground shadow-md ring-1 ring-foreground/10 hover:text-destructive"
    >
      <X className="size-3.5" aria-hidden />
    </button>
  );
}
