import { memo, useMemo } from "react";
import type { Node, NodeProps } from "@xyflow/react";
import { COLUMN_WIDTH } from "../lib/layout";
import { plural } from "../lib/order";
import { roughRect } from "../lib/rough";
import { RoughBackdrop, RoughShape } from "./RoughShape";

/** The dashed boxes that aren't slices: the "Unassigned" group and each "Later" item. */
export type GroupNodeType = Node<
  { id: string; height: number } & ({ kind: "unassigned"; count: number } | { kind: "later"; after: string }),
  "group"
>;

export const GroupNode = memo(function GroupNode({ data }: NodeProps<GroupNodeType>) {
  const paths = useMemo(() => roughRect(data.id, COLUMN_WIDTH, data.height, { roughness: 0.9 }), [data.id, data.height]);

  return (
    <div className="relative" style={{ width: COLUMN_WIDTH, height: data.height }}>
      <RoughBackdrop width={COLUMN_WIDTH} height={data.height}>
        <RoughShape paths={paths} strokeClassName="stroke-muted-foreground/70" dashed />
      </RoughBackdrop>
      <div className="relative flex h-full flex-col justify-center gap-0.5 px-3 py-2">
        <p className="text-[13px] leading-tight font-semibold text-muted-foreground">
          {data.kind === "unassigned" ? "Unassigned" : "Later"}
        </p>
        <p className="truncate text-[11px] text-muted-foreground">
          {data.kind === "unassigned" ? plural(data.count, "requirement") : `after ${data.after}`}
        </p>
      </div>
    </div>
  );
});
