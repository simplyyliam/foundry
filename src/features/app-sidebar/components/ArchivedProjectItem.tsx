import { ArchiveRestore, Trash2 } from "lucide-react";
import { useState } from "react";
import { unarchiveProject, type Project } from "@/features/projects";
import { SidebarMenuItem } from "@/shared";
import { DeleteProjectDialog } from "./DeleteProjectDialog";
import { ProjectActionsMenu } from "./ProjectActionsMenu";

/** An archived project: not navigable; "more" offers Unarchive and Delete. */
export function ArchivedProjectItem({ project }: { project: Project }) {
  const [deleting, setDeleting] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const unarchive = async () => {
    setActionError(null);
    const outcome = await unarchiveProject(project.id);
    if (!outcome.ok) setActionError(outcome.error.message);
  };

  return (
    <SidebarMenuItem>
      <div
        title={project.repoPath}
        className="flex h-8 items-center rounded-xl px-3 pr-8 text-[12px] text-muted-foreground transition-colors group-hover/menu-item:bg-sidebar-accent/60"
      >
        <span className="truncate">{project.name}</span>
      </div>
      <ProjectActionsMenu
        projectName={project.name}
        actions={[
          {
            id: "unarchive",
            label: "Unarchive",
            icon: ArchiveRestore,
            onSelect: () => void unarchive(),
          },
          {
            id: "delete",
            label: "Delete…",
            icon: Trash2,
            destructive: true,
            onSelect: () => {
              setActionError(null);
              setDeleting(true);
            },
          },
        ]}
      />
      {actionError && (
        <p role="alert" className="px-3 pt-0.5 pb-1 text-[11.5px] leading-snug text-destructive">
          {actionError}
        </p>
      )}
      <DeleteProjectDialog project={project} open={deleting} onOpenChange={setDeleting} />
    </SidebarMenuItem>
  );
}
