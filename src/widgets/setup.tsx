import { AgentsMdSection } from "@/features/setup";

export interface SetupProps {
  /** The project from the route. */
  projectId: string;
}

/** The project's Setup page: a vertical stack of sections (AGENTS.md; Team comes next). */
export function Setup({ projectId }: SetupProps) {
  return (
    <div key={projectId} className="h-full w-full overflow-y-auto bg-background">
      <div className="mx-auto flex max-w-3xl flex-col gap-12 px-6 pt-6 pb-16">
        <AgentsMdSection projectId={projectId} />
      </div>
    </div>
  );
}
