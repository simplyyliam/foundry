import {
  LoadFailed,
  NoticeBanner,
  RequirementsHeader,
  RequirementsList,
  SummaryBanner,
  usePlan,
  useRequirementActions,
} from "@/features/requirements";

export interface RequirementsProps {
  /** The project from the route. */
  projectId: string;
}

/** Every requirement, grouped by slice, with assign / confirm / edit / delete. */
export function Requirements({ projectId }: RequirementsProps) {
  return <ProjectRequirements key={projectId} projectId={projectId} />;
}

function ProjectRequirements({ projectId }: RequirementsProps) {
  const { load, notice, dismissNotice, mutate } = usePlan(projectId);
  const actions = useRequirementActions(projectId, mutate);

  if (load.status === "loading") return <div className="h-full w-full bg-background" />;
  if (load.status === "error") return <LoadFailed message={load.error.message} />;

  return (
    <div className="relative h-full w-full bg-background">
      <div className="h-full overflow-y-auto">
        <div className="mx-auto flex max-w-3xl flex-col gap-5 px-6 pt-6 pb-16">
          <RequirementsHeader plan={load.plan} actions={actions} />
          <SummaryBanner plan={load.plan} summary={load.summary} />
          <RequirementsList plan={load.plan} actions={actions} />
        </div>
      </div>
      <div className="pointer-events-none absolute inset-x-0 top-3 flex justify-center px-4">
        <NoticeBanner notice={notice} onDismiss={dismissNotice} />
      </div>
    </div>
  );
}
