import { useCallback, useMemo, useState } from "react";
import {
  Background,
  BackgroundVariant,
  ReactFlow,
  ReactFlowProvider,
  type Connection,
  type EdgeChange,
  type EdgeTypes,
  type IsValidConnection,
  type NodeTypes,
  type OnBeforeDelete,
} from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import type { Plan } from "@/lib/bindings";
import { COLUMN_WIDTH, FIT_VIEW, layoutBlueprint } from "../lib/layout";
import { CanvasControls } from "./CanvasControls";
import { DependencyEdge, type DependencyEdgeType } from "./DependencyEdge";
import { GroupNode, type GroupNodeType } from "./GroupNode";
import { LanesNode, type LanesNodeType } from "./LanesNode";
import { SliceNode, type SliceNodeType } from "./SliceNode";

type BlueprintNode = LanesNodeType | SliceNodeType | GroupNodeType;

const nodeTypes: NodeTypes = { lanes: LanesNode, slice: SliceNode, group: GroupNode };
const edgeTypes: EdgeTypes = { dependency: DependencyEdge };

const edgeId = (dependency: string, slice: string) => `${dependency}->${slice}`;
const parseEdgeId = (id: string) => {
  const [dependency, slice] = id.split("->");
  return { dependency, slice };
};

export interface BlueprintCanvasProps {
  plan: Plan;
  /** The slice whose sheet is open (drawn with a heavier outline). */
  openSliceId: string | null;
  onOpenSlice: (sliceId: string) => void;
  /** The "Later" item whose sheet is open. */
  openDeferredId: string | null;
  onOpenDeferred: (deferredId: string) => void;
  /**
   * Replaces one slice's dependencies (the full list). The canvas only ever draws what the
   * plan says, so a refused change leaves it exactly as it was.
   */
  onSetDependencies: (sliceId: string, dependsOn: string[]) => Promise<void>;
}

export function BlueprintCanvas(props: BlueprintCanvasProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}

function Canvas({ plan, openSliceId, onOpenSlice, openDeferredId, onOpenDeferred, onSetDependencies }: BlueprintCanvasProps) {
  const layout = useMemo(() => layoutBlueprint(plan), [plan]);
  const [selectedEdges, setSelectedEdges] = useState<ReadonlySet<string>>(new Set());

  const nodes = useMemo<BlueprintNode[]>(() => {
    const fixed = { draggable: false, selectable: false, focusable: false } as const;
    const lanes: LanesNodeType = {
      id: "lanes",
      type: "lanes",
      position: { x: 0, y: 0 },
      data: { layout },
      width: layout.width,
      height: layout.height,
      zIndex: -1,
      connectable: false,
      // Lets drags fall through to the pane, so they pan.
      style: { pointerEvents: "none" },
      ...fixed,
    };
    const boxes = layout.boxes.map((box): SliceNodeType | GroupNodeType =>
      box.kind === "slice"
        ? {
            id: box.id,
            type: "slice",
            position: { x: box.x, y: box.y },
            width: COLUMN_WIDTH,
            height: box.height,
            data: { slice: box.slice, count: box.count, height: box.height, open: box.id === openSliceId, onOpen: onOpenSlice },
            ...fixed,
          }
        : {
            id: box.id,
            type: "group",
            position: { x: box.x, y: box.y },
            width: COLUMN_WIDTH,
            height: box.height,
            connectable: false,
            data:
              box.kind === "unassigned"
                ? { id: box.id, height: box.height, kind: "unassigned", count: box.count }
                : {
                    id: box.id,
                    height: box.height,
                    kind: "later",
                    deferredId: box.deferredId,
                    label: box.label,
                    after: box.after,
                    open: box.deferredId === openDeferredId,
                    onOpen: onOpenDeferred,
                  },
            ...fixed,
          },
    );
    return [lanes, ...boxes];
  }, [layout, openSliceId, onOpenSlice, openDeferredId, onOpenDeferred]);

  const removeEdges = useCallback(
    async (ids: readonly string[]) => {
      // One setDependencies per affected slice, each with that slice's full remaining list.
      const bySlice = new Map<string, Set<string>>();
      for (const id of ids) {
        const { dependency, slice } = parseEdgeId(id);
        bySlice.set(slice, (bySlice.get(slice) ?? new Set()).add(dependency));
      }
      for (const [sliceId, removed] of bySlice) {
        const slice = plan.slices.find((candidate) => candidate.id === sliceId);
        if (!slice) continue;
        await onSetDependencies(sliceId, slice.dependsOn.filter((dependency) => !removed.has(dependency)));
      }
      setSelectedEdges(new Set());
    },
    [plan, onSetDependencies],
  );

  const removeOne = useCallback((id: string) => void removeEdges([id]), [removeEdges]);

  const edges = useMemo<DependencyEdgeType[]>(
    () =>
      plan.slices.flatMap((slice) =>
        slice.dependsOn
          .filter((dependency) => plan.slices.some((candidate) => candidate.id === dependency))
          .map((dependency) => {
            const id = edgeId(dependency, slice.id);
            return {
              id,
              type: "dependency",
              source: dependency,
              target: slice.id,
              selected: selectedEdges.has(id),
              data: { onRemove: removeOne },
            };
          }),
      ),
    [plan, selectedEdges, removeOne],
  );

  const onEdgesChange = useCallback((changes: EdgeChange<DependencyEdgeType>[]) => {
    // Only selection is local; removals go through the backend (onBeforeDelete).
    setSelectedEdges((previous) => {
      const next = new Set(previous);
      for (const change of changes) {
        if (change.type !== "select") continue;
        if (change.selected) next.add(change.id);
        else next.delete(change.id);
      }
      return next;
    });
  }, []);

  const onBeforeDelete = useCallback<OnBeforeDelete<BlueprintNode, DependencyEdgeType>>(
    async ({ edges: doomed }) => {
      if (doomed.length > 0) void removeEdges(doomed.map((edge) => edge.id));
      // Never let React Flow delete locally: the plan the backend returns redraws the canvas.
      return false;
    },
    [removeEdges],
  );

  const isValidConnection = useCallback<IsValidConnection>(
    (connection) => {
      if (!connection.source || !connection.target || connection.source === connection.target) return false;
      const target = plan.slices.find((slice) => slice.id === connection.target);
      const sourceIsSlice = plan.slices.some((slice) => slice.id === connection.source);
      return Boolean(target && sourceIsSlice && !target.dependsOn.includes(connection.source));
    },
    [plan],
  );

  const onConnect = useCallback(
    (connection: Connection) => {
      const target = plan.slices.find((slice) => slice.id === connection.target);
      if (!target || target.dependsOn.includes(connection.source)) return;
      void onSetDependencies(target.id, [...target.dependsOn, connection.source]);
    },
    [plan, onSetDependencies],
  );

  return (
    <ReactFlow<BlueprintNode, DependencyEdgeType>
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      onEdgesChange={onEdgesChange}
      onBeforeDelete={onBeforeDelete}
      onConnect={onConnect}
      isValidConnection={isValidConnection}
      deleteKeyCode={["Delete", "Backspace"]}
      nodesDraggable={false}
      nodesConnectable
      elementsSelectable
      selectNodesOnDrag={false}
      panOnScroll
      zoomOnScroll={false}
      zoomOnDoubleClick={false}
      minZoom={0.15}
      maxZoom={2}
      connectionLineStyle={{ stroke: "var(--slice)", strokeWidth: 1.6, strokeDasharray: "5 4" }}
      // Once, on first load (the widget is keyed by project). Boxes carry their own sizes.
      fitView
      fitViewOptions={FIT_VIEW}
      className="bg-background"
    >
      <Background
        variant={BackgroundVariant.Dots}
        gap={18}
        size={1.3}
        bgColor="transparent"
        color="color-mix(in oklch, var(--muted-foreground) 35%, transparent)"
      />
      <CanvasControls />
    </ReactFlow>
  );
}
