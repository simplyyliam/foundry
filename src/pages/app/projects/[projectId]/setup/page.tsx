import { ProjectRoute, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Setup } from "@/widgets";

function SetupPage({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Setup" }]);
  return <Setup projectId={project.id} />;
}

/** /projects/:projectId/setup */
export default function SetupRoute() {
  return <ProjectRoute>{(project) => <SetupPage project={project} />}</ProjectRoute>;
}
