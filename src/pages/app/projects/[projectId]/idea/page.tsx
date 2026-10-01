import { Navigate, useParams } from "react-router-dom";
import { useProject, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Document } from "@/widgets";

function Idea({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Idea" }]);
  return <Document projectId={project.id} />;
}

/** /projects/:projectId/idea — the project's document. */
export default function IdeaPage() {
  const { projectId } = useParams();
  const project = useProject(projectId);
  if (!project) return <Navigate to="/home" replace />;
  return <Idea project={project} />;
}
