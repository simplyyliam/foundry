import { Navigate, useParams } from "react-router-dom";
import { useProject, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";

function Blueprint({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Blueprint" }]);

  // Placeholder until the blueprints page is built.
  return (
    <div className="flex h-full w-full items-center justify-center bg-background text-sm text-muted-foreground">
      Blueprint
    </div>
  );
}

/** /projects/:projectId/blueprints */
export default function BlueprintsPage() {
  const { projectId } = useParams();
  const project = useProject(projectId);
  if (!project) return <Navigate to="/home" replace />;
  return <Blueprint project={project} />;
}
