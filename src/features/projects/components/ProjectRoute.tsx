import { useEffect, useState, type ReactNode } from "react";
import { Navigate, useParams } from "react-router-dom";
import { openProject, useProject } from "../model/store";
import type { Project } from "../model/project";

interface ProjectRouteProps {
  /** Renders the page once the project is known and opened. */
  children: (project: Project) => ReactNode;
}

/**
 * Wraps every /projects/:projectId/* page: waits for the first project list, sends unknown
 * or archived projects to Home, and opens the project on the backend (active project +
 * lastOpened). If opening fails (e.g. folder_missing), shows the backend's message.
 */
export function ProjectRoute({ children }: ProjectRouteProps) {
  const { projectId } = useParams();
  const { project, loaded } = useProject(projectId);
  const [openError, setOpenError] = useState<{ id: string; message: string } | null>(null);
  const available = project !== undefined && !project.archived;

  useEffect(() => {
    if (!available || !projectId) return;
    let cancelled = false;
    openProject(projectId).then((outcome) => {
      if (!cancelled && !outcome.ok) setOpenError({ id: projectId, message: outcome.error.message });
    });
    return () => {
      cancelled = true;
    };
  }, [available, projectId]);

  if (!loaded) return null;
  if (!project || project.archived) return <Navigate to="/home" replace />;

  if (openError?.id === project.id) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-background px-8">
        <p role="alert" className="max-w-md text-center text-sm text-muted-foreground">
          {openError.message}
        </p>
      </div>
    );
  }
  return children(project);
}
