import { Navigate, useParams } from "react-router-dom";
import { PROJECT_SECTIONS, projectSectionPath } from "@/features/projects";

/** /projects/:projectId has no page of its own (yet): open the first section. */
export default function ProjectPage() {
  const { projectId = "" } = useParams();
  return <Navigate to={projectSectionPath(projectId, PROJECT_SECTIONS[0])} replace />;
}
