import type { Plan, PlanRequirement, SliceView } from "@/lib/bindings";
import { orderSlices, requirementsBySlice } from "./order";

/**
 * Blueprint geometry, in canvas units. One row band per requirement, grouped by slice
 * (slices by layer, then name), then the unassigned requirements, then deferred items.
 * Each slice is a box in its layer's column spanning exactly its rows, which makes the
 * staircase. Everything is derived from the plan: nothing is stored.
 */

export const LOOK_ACROSS_WIDTH = 320;
export const HEADER_HEIGHT = 44;
export const ROW_HEIGHT = 60;
const ROW_GAP = 4;
const GROUP_GAP = 18;
const COLUMN_START = LOOK_ACROSS_WIDTH + 48;
export const COLUMN_WIDTH = 216;
/** Fit-to-view: the whole blueprint, never zoomed past 100%. */
export const FIT_VIEW = { padding: 0.06, maxZoom: 1 };
const COLUMN_GAP = 80;
const BOTTOM_PADDING = 20;
const RIGHT_PADDING = 32;

export type RowKind = "requirement" | "unassigned" | "later" | "empty";

export interface Row {
  key: string;
  kind: RowKind;
  y: number;
  /** Look Across text; empty for a slice with no requirements. */
  text: string;
  requirement?: PlanRequirement;
}

export type Box =
  | { kind: "slice"; id: string; slice: SliceView; column: number; x: number; y: number; height: number; count: number }
  | { kind: "unassigned"; id: string; column: number; x: number; y: number; height: number; count: number }
  | { kind: "later"; id: string; column: number; x: number; y: number; height: number; after: string };

export interface BlueprintLayout {
  rows: Row[];
  boxes: Box[];
  width: number;
  height: number;
  columns: number;
}

export const columnX = (column: number) => COLUMN_START + column * (COLUMN_WIDTH + COLUMN_GAP);

export function layoutBlueprint(plan: Plan): BlueprintLayout {
  const rows: Row[] = [];
  const boxes: Box[] = [];
  const bySlice = requirementsBySlice(plan);
  const slices = orderSlices(plan.slices);
  const layerOf = new Map(slices.map((slice) => [slice.id, slice.layer]));
  let y = HEADER_HEIGHT;
  let columns = 1;

  /** Lays out a run of rows; returns the run's top and height. */
  const run = (entries: Omit<Row, "y">[]) => {
    const top = y;
    for (const entry of entries) {
      rows.push({ ...entry, y });
      y += ROW_HEIGHT + ROW_GAP;
    }
    const height = y - ROW_GAP - top;
    y += GROUP_GAP - ROW_GAP;
    return { top, height };
  };

  for (const slice of slices) {
    const requirements = bySlice.get(slice.id) ?? [];
    const entries: Omit<Row, "y">[] =
      requirements.length > 0
        ? requirements.map((requirement) => ({ key: requirement.id, kind: "requirement", text: requirement.text, requirement }))
        : // A slice with no requirements still needs a slot for its short box.
          [{ key: `empty:${slice.id}`, kind: "empty", text: "" }];
    const { top, height } = run(entries);
    columns = Math.max(columns, slice.layer + 1);
    boxes.push({
      kind: "slice",
      id: slice.id,
      slice,
      column: slice.layer,
      x: columnX(slice.layer),
      y: top,
      height,
      count: requirements.length,
    });
  }

  const unassigned = bySlice.get(null) ?? [];
  if (unassigned.length > 0) {
    const { top, height } = run(
      unassigned.map((requirement) => ({ key: requirement.id, kind: "unassigned", text: requirement.text, requirement })),
    );
    boxes.push({ kind: "unassigned", id: "unassigned", column: 0, x: columnX(0), y: top, height, count: unassigned.length });
  }

  if (plan.deferred.length > 0) {
    const top = y;
    for (const item of plan.deferred) {
      const anchor = plan.slices.find((slice) => slice.id === item.anchoredTo);
      // "Later" sits one column after the slice it follows.
      const column = (layerOf.get(item.anchoredTo) ?? -1) + 1;
      columns = Math.max(columns, column + 1);
      rows.push({ key: `later:${item.id}`, kind: "later", y, text: item.label });
      boxes.push({
        kind: "later",
        id: `later:${item.id}`,
        column,
        x: columnX(column),
        y,
        height: ROW_HEIGHT,
        after: anchor?.name ?? "a removed slice",
      });
      y += ROW_HEIGHT + ROW_GAP;
    }
    if (y > top) y += GROUP_GAP - ROW_GAP;
  }

  const height = Math.max(y - GROUP_GAP + BOTTOM_PADDING, HEADER_HEIGHT + ROW_HEIGHT);
  const width = columnX(columns - 1) + COLUMN_WIDTH + RIGHT_PADDING;
  return { rows, boxes, width, height, columns };
}
