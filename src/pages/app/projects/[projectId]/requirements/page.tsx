import { ProjectRoute, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Requirements } from "@/widgets";

function RequirementsPage({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Requirements" }]);
  return <Requirements projectId={project.id} />;
}

/** /projects/:projectId/requirements */
export default function RequirementsRoute() {
  return <ProjectRoute>{(project) => <RequirementsPage project={project} />}</ProjectRoute>;
}
