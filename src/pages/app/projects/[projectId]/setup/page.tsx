import { ProjectRoute, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Setup, SetupCommit, TeamComposition } from "@/widgets";

function SetupPage({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Setup" }]);
  // Widgets can't import each other, so the page stacks the sections: AGENTS.md, Team, then
  // Commit setup (which shows only while setup files need committing).
  return (
    <div className="h-full w-full overflow-y-auto bg-background">
      <div className="mx-auto flex max-w-3xl flex-col gap-12 px-6 pt-6 pb-16">
        <Setup projectId={project.id} />
        <TeamComposition projectId={project.id} />
        <SetupCommit projectId={project.id} />
      </div>
    </div>
  );
}

/** /projects/:projectId/setup */
export default function SetupRoute() {
  return <ProjectRoute>{(project) => <SetupPage project={project} />}</ProjectRoute>;
}
