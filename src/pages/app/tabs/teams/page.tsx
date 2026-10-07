import { FolderOpen } from "lucide-react";
import { Navigate } from "react-router-dom";
import { useProjectsSnapshot, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";

/** Most recently opened; never-opened ones count as oldest, and ties go to the newest added. */
function mostRecent(projects: readonly Project[]): Project | undefined {
  let best: Project | undefined;
  for (const project of projects) {
    if (project.archived) continue;
    if (!best || (project.lastOpened ?? -1) >= (best.lastOpened ?? -1)) best = project;
  }
  return best;
}

/** /teams: goes to the most recently opened project's team, or asks for a project. */
export default function TeamsIndexRoute() {
  usePageHeader({ title: "Teams" });
  const { status, projects, error } = useProjectsSnapshot();
  if (status === "loading") return null;

  const project = mostRecent(projects);
  if (project) return <Navigate to={`/teams/${project.id}`} replace />;

  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-3 bg-background px-8 text-center">
      <FolderOpen aria-hidden className="size-6 text-muted-foreground" />
      <div className="flex flex-col gap-1">
        <p className="text-[14px] font-medium">Create or open a project</p>
        <p className="max-w-sm text-[13px] text-muted-foreground">
          {status === "error" && error
            ? error.message
            : "Use the + beside Projects in the sidebar. Its team shows up here once it's confirmed."}
        </p>
      </div>
    </div>
  );
}
