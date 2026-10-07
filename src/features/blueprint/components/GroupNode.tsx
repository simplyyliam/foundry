import { memo, useMemo, type ReactNode } from "react";
import type { Node, NodeProps } from "@xyflow/react";
import { cn } from "@/lib/utils";
import { usePressScale } from "@/shared";
import { COLUMN_WIDTH } from "../lib/layout";
import { plural } from "../lib/order";
import { roughRect } from "../lib/rough";
import { RoughBackdrop, RoughShape } from "./RoughShape";

/** The dashed boxes that aren't slices: the "Unassigned" group and each "Later" item. */
export type GroupNodeType = Node<
  { id: string; height: number } & (
    | { kind: "unassigned"; count: number }
    | {
        kind: "later";
        deferredId: string;
        label: string;
        after: string;
        open: boolean;
        onOpen: (deferredId: string) => void;
      }
  ),
  "group"
>;

export const GroupNode = memo(function GroupNode({ data }: NodeProps<GroupNodeType>) {
  const later = data.kind === "later";
  const open = later && data.open;
  const paths = useMemo(() => roughRect(data.id, COLUMN_WIDTH, data.height, { roughness: 0.9 }), [data.id, data.height]);

  const body = (
    <>
      <RoughBackdrop width={COLUMN_WIDTH} height={data.height}>
        <RoughShape
          paths={paths}
          strokeClassName={cn("transition-[stroke-width]", open ? "stroke-foreground/70" : "stroke-muted-foreground/70")}
          strokeWidth={open ? 2 : undefined}
          dashed
        />
      </RoughBackdrop>
      <span className="relative flex h-full flex-col justify-center gap-0.5 px-3 py-2">
        <span className="text-[13px] leading-tight font-semibold text-muted-foreground">{later ? "Later" : "Unassigned"}</span>
        <span className="truncate text-[11px] text-muted-foreground">
          {later ? `after ${data.after}` : plural(data.count, "requirement")}
        </span>
      </span>
    </>
  );

  return (
    <div className="relative" style={{ width: COLUMN_WIDTH, height: data.height }}>
      {later ? (
        <LaterButton label={`Later: ${data.label}, after ${data.after}. Open details`} onClick={() => data.onOpen(data.deferredId)}>
          {body}
        </LaterButton>
      ) : (
        body
      )}
    </div>
  );
});

/**
 * Clickable like a slice box. `nodrag nopan` keeps a press on it from panning the canvas, and
 * `pointer-events-auto` undoes React Flow turning pointer events off on unselectable nodes.
 */
function LaterButton({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  const pressRef = usePressScale<HTMLButtonElement>({ scale: 0.985 });
  return (
    <button
      ref={pressRef}
      type="button"
      onClick={onClick}
      aria-label={label}
      className="nodrag nopan pointer-events-auto absolute inset-0 rounded-xl text-left outline-none hover:bg-muted/40 focus-visible:ring-3 focus-visible:ring-ring/40"
    >
      {children}
    </button>
  );
}
