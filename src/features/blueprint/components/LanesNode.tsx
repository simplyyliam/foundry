import { memo, useMemo } from "react";
import type { Node, NodeProps } from "@xyflow/react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { HEADER_HEIGHT, LOOK_ACROSS_WIDTH, ROW_HEIGHT, type BlueprintLayout } from "../lib/layout";
import { roughRect } from "../lib/rough";
import { RoughBackdrop, RoughShape } from "./RoughShape";

export type LanesNodeType = Node<{ layout: BlueprintLayout }, "lanes">;

/**
 * Everything behind the slice boxes: the row bands across the whole canvas and the tall
 * "Look Across" column with each row's text. One node at the origin, below edges and boxes,
 * ignoring the pointer so dragging anywhere on it pans.
 */
export const LanesNode = memo(function LanesNode({ data }: NodeProps<LanesNodeType>) {
  const { rows, width, height } = data.layout;
  // Keyed by height only: the column's outline only changes when the plan grows or shrinks.
  const column = useMemo(
    () => roughRect("look-across", LOOK_ACROSS_WIDTH, height, { fill: "currentColor", fillStyle: "solid", roughness: 0.9 }),
    [height],
  );
  const fill = useMemo(() => column.filter((path) => path.role === "fill"), [column]);
  const outline = useMemo(() => column.filter((path) => path.role === "stroke"), [column]);

  return (
    <div className="relative" style={{ width, height }}>
      <RoughBackdrop width={LOOK_ACROSS_WIDTH} height={height}>
        <RoughShape paths={fill} fillClassName="fill-background" />
      </RoughBackdrop>

      {rows.map((row) =>
        row.kind === "empty" ? null : (
          <div
            key={row.key}
            className={cn(
              "absolute left-0 rounded-lg",
              row.kind === "requirement" ? "bg-muted" : "border border-dashed border-muted-foreground/35",
            )}
            style={{ top: row.y, width, height: ROW_HEIGHT }}
          />
        ),
      )}

      <RoughBackdrop width={LOOK_ACROSS_WIDTH} height={height}>
        <RoughShape paths={outline} strokeClassName="stroke-muted-foreground/70" />
      </RoughBackdrop>

      <p
        className="absolute left-0 flex items-center px-5 text-[13px] font-semibold tracking-tight text-foreground"
        style={{ height: HEADER_HEIGHT, width: LOOK_ACROSS_WIDTH }}
      >
        Look Across
      </p>

      {rows.map((row) =>
        row.text ? (
          <div
            key={row.key}
            className="absolute left-0 flex items-center gap-2 pr-4 pl-5"
            style={{ top: row.y, width: LOOK_ACROSS_WIDTH, height: ROW_HEIGHT }}
          >
            {row.requirement?.projectWide && (
              <Badge variant="outline" className="h-4 shrink-0 px-1.5 text-[10px]" title="A project rule: applies to every slice">
                rule
              </Badge>
            )}
            <p
              className={cn(
                "line-clamp-3 text-[12px] leading-4 text-pretty",
                row.kind === "later" ? "text-muted-foreground" : "text-foreground/85",
              )}
            >
              {row.text}
            </p>
          </div>
        ) : null,
      )}
    </div>
  );
});
