import { memo, useMemo } from "react";
import { Handle, Position, useConnection, type Node, type NodeProps } from "@xyflow/react";
import { CheckCircle2 } from "lucide-react";
import { cn } from "@/lib/utils";
import type { SliceView } from "@/lib/bindings";
import { usePressScale } from "@/shared";
import { COLUMN_WIDTH, ROW_HEIGHT } from "../lib/layout";
import { plural } from "../lib/order";
import { roughRect } from "../lib/rough";
import { RoughBackdrop, RoughShape } from "./RoughShape";
import { StatusChip } from "./StatusChip";

export type SliceNodeType = Node<
  { slice: SliceView; count: number; height: number; open: boolean; onOpen: (sliceId: string) => void },
  "slice"
>;

/**
 * A slice box: hand-drawn orange outline behind plain DOM text. The left edge is one tall
 * target handle (drop a dependency anywhere along it); the dot on the right starts a drag.
 */
export const SliceNode = memo(function SliceNode({ data }: NodeProps<SliceNodeType>) {
  const { slice, count, height, open, onOpen } = data;
  const pressRef = usePressScale<HTMLButtonElement>({ scale: 0.985 });
  const paths = useMemo(
    () => roughRect(slice.id, COLUMN_WIDTH, height, { fill: "currentColor", fillStyle: "solid" }),
    [slice.id, height],
  );
  const tall = height >= ROW_HEIGHT * 2;
  const connecting = useConnection((connection) => connection.inProgress && connection.fromNode.id !== slice.id);

  return (
    <div className="group/slice relative" style={{ width: COLUMN_WIDTH, height }}>
      {/* pointer-events-auto: React Flow turns them off on nodes that are neither selectable
          nor draggable, and the button would inherit that. */}
      <button
        ref={pressRef}
        type="button"
        onClick={() => onOpen(slice.id)}
        aria-label={`${slice.name}: ${plural(count, "requirement")}, ${slice.status.replace("_", " ")}. Open details`}
        className="nodrag nopan pointer-events-auto absolute inset-0 rounded-xl text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/40"
      >
        <RoughBackdrop width={COLUMN_WIDTH} height={height}>
          <RoughShape
            paths={paths}
            fillClassName="fill-slice-fill"
            strokeClassName={cn("stroke-slice transition-[stroke-width]", open && "stroke-slice-foreground")}
            strokeWidth={open ? 2.2 : 1.5}
          />
        </RoughBackdrop>
        <span className="relative flex h-full flex-col justify-between gap-1 px-3 py-2">
          <span
            className={cn(
              "text-[13px] leading-[1.25] font-semibold text-slice-foreground",
              tall ? "line-clamp-3" : "line-clamp-1",
            )}
          >
            {slice.name}
          </span>
          <span className="flex min-w-0 items-center gap-1.5">
            <StatusChip status={slice.status} />
            <span className="truncate text-[11px] text-slice-foreground/80 tabular-nums">{plural(count, "requirement")}</span>
            {slice.ready && (
              <span
                title="Every dependency is done"
                className="ml-auto flex shrink-0 items-center gap-0.5 text-[10px] font-medium text-slice-foreground"
              >
                <CheckCircle2 className="size-3" aria-hidden />
                ready
              </span>
            )}
          </span>
        </span>
      </button>
      {/* After the button so it sits on top. While a connection is being dragged it covers the
          whole box, so a dependency can be dropped anywhere on it. */}
      <Handle
        type="target"
        position={Position.Left}
        isConnectableStart={false}
        style={
          connecting
            ? { top: 0, left: 0, width: "100%", height: "100%", transform: "none" }
            : { top: 0, height: "100%", width: 12, transform: "translateX(-50%)" }
        }
        className="rounded-xl! border-0! bg-transparent! [&.connectingto.valid]:bg-slice/15!"
      />
      <Handle
        type="source"
        position={Position.Right}
        title="Drag to a later slice to add a dependency"
        className="size-3! rounded-full! border-2! border-background! bg-slice! opacity-60 transition-[opacity,scale] group-hover/slice:opacity-100 hover:scale-125"
      />
    </div>
  );
});
