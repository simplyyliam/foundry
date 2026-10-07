import { ProjectRoute, useProjects, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Teams } from "@/widgets";

function TeamsPage({ project }: { project: Project }) {
  const projects = useProjects();
  usePageHeader({
    title: "Teams",
    subtitle: "Working on",
    subtitleMenu: {
      label: "Switch project",
      items: projects.filter((other) => !other.archived).map((other) => ({ title: other.name, to: `/teams/${other.id}` })),
      current: `/teams/${project.id}`,
    },
  });
  return (
    <div className="h-full w-full overflow-y-auto bg-background">
      <div className="px-5 pt-1 pb-5">
        <Teams projectId={project.id} />
      </div>
    </div>
  );
}

/** /teams/:projectId */
export default function TeamsRoute() {
  return <ProjectRoute>{(project) => <TeamsPage project={project} />}</ProjectRoute>;
}
