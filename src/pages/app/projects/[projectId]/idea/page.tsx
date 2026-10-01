import { ProjectRoute, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Document } from "@/widgets";

function Idea({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Idea" }]);
  return <Document projectId={project.id} />;
}

/** /projects/:projectId/idea — the project's document. */
export default function IdeaPage() {
  return <ProjectRoute>{(project) => <Idea project={project} />}</ProjectRoute>;
}
