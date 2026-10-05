import { PROJECT_SECTIONS, ProjectRoute, projectSectionPath, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Blueprint } from "@/widgets";

const REQUIREMENTS = PROJECT_SECTIONS.find((section) => section.id === "requirements")!;

function BlueprintPage({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Blueprint" }]);
  return <Blueprint projectId={project.id} requirementsHref={projectSectionPath(project.id, REQUIREMENTS)} />;
}

/** /projects/:projectId/blueprints */
export default function BlueprintsPage() {
  return <ProjectRoute>{(project) => <BlueprintPage project={project} />}</ProjectRoute>;
}
