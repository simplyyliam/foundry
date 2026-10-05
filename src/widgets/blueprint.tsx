import { useCallback, useState } from "react";
import {
  AddSliceDialog,
  BlueprintCanvas,
  BlueprintHeader,
  LoadFailed,
  NoSlices,
  NoticeBanner,
  SliceSheet,
  usePlan,
  usePlanActions,
} from "@/features/blueprint";

export interface BlueprintProps {
  /** The project from the route. */
  projectId: string;
  /** Where the header's Requirements link goes. */
  requirementsHref: string;
}

/** The plan as a canvas: rows of requirements, slice boxes in a staircase, dependency arrows. */
export function Blueprint(props: BlueprintProps) {
  // Keyed by project: switching project starts from a clean load (and a fresh fit-to-view).
  return <ProjectBlueprint key={props.projectId} {...props} />;
}

function ProjectBlueprint({ projectId, requirementsHref }: BlueprintProps) {
  const { load, notice, dismissNotice, report, mutate } = usePlan(projectId);
  const actions = usePlanActions(projectId, mutate);
  const [openSliceId, setOpenSliceId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const setDependencies = useCallback(
    async (sliceId: string, dependsOn: string[]) => {
      const outcome = await actions.setDependencies(sliceId, dependsOn);
      // stale_revision already raised its own notice; anything else (a cycle) is shown as-is.
      if (!outcome.ok && outcome.error.code !== "stale_revision") report(outcome.error);
    },
    [actions, report],
  );

  if (load.status === "loading") return <div className="h-full w-full bg-background" />;
  if (load.status === "error") return <LoadFailed message={load.error.message} />;
  const { plan } = load;

  return (
    <div className="flex h-full w-full flex-col bg-background">
      <BlueprintHeader plan={plan} requirementsHref={requirementsHref} onAddSlice={() => setAdding(true)} />
      <div className="relative min-h-0 flex-1">
        <BlueprintCanvas
          plan={plan}
          openSliceId={openSliceId}
          onOpenSlice={setOpenSliceId}
          onSetDependencies={setDependencies}
        />
        <div className="pointer-events-none absolute inset-x-0 top-3 flex justify-center px-4">
          <NoticeBanner notice={notice} onDismiss={dismissNotice} />
        </div>
        {plan.slices.length === 0 && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center p-6">
            <NoSlices onAddSlice={() => setAdding(true)} />
          </div>
        )}
      </div>
      <SliceSheet plan={plan} sliceId={openSliceId} actions={actions} onClose={() => setOpenSliceId(null)} />
      <AddSliceDialog open={adding} onOpenChange={setAdding} actions={actions} onAdded={setOpenSliceId} />
    </div>
  );
}
