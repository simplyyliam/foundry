import { Archive, Pencil } from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import { useMatch, useNavigate } from "react-router-dom";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  archiveProject,
  PROJECT_SECTIONS,
  projectPath,
  projectSectionPath,
  type Project,
} from "@/features/projects";
import { SidebarMenuButton, SidebarMenuItem, SidebarMenuSub } from "@/shared";
import { ProjectActionsMenu } from "./ProjectActionsMenu";
import { ProjectNameEditor } from "./ProjectNameEditor";
import { ProjectSectionLink } from "./ProjectSectionLink";

interface ProjectMenuItemProps {
  project: Project;
  /** Appeared after the sidebar first loaded: fades in instead of just appearing. */
  isNew: boolean;
}

/** An active project: clicking the row shows or hides its pages; "more" offers Rename and Archive. */
export function ProjectMenuItem({ project, isNew }: ProjectMenuItemProps) {
  const navigate = useNavigate();
  const isInside = useMatch({ path: projectPath(project.id), end: false }) !== null;
  const [renaming, setRenaming] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  // Opens whenever the route enters the project; clicking the row still toggles it.
  const [open, setOpen] = useState(isInside);
  const [wasInside, setWasInside] = useState(isInside);
  if (isInside !== wasInside) {
    setWasInside(isInside);
    if (isInside) setOpen(true);
  }

  const archive = async () => {
    setActionError(null);
    const outcome = await archiveProject(project.id);
    // Shown as the backend wrote it (e.g. a running session); nothing else happens.
    if (!outcome.ok) return setActionError(outcome.error.message);
    if (isInside) navigate("/home");
  };

  return (
    <SidebarMenuItem>
      <motion.div
        initial={isNew ? { opacity: 0, y: -4, filter: "blur(2px)" } : false}
        animate={{ opacity: 1, y: 0, filter: "blur(0px)", transitionEnd: { filter: "none" } }}
        transition={{ duration: 0.3, ease: [0.32, 0.72, 0, 1] }}
      >
        <Collapsible open={open} onOpenChange={setOpen}>
          {renaming ? (
            <ProjectNameEditor project={project} onDone={() => setRenaming(false)} />
          ) : (
            <>
              <CollapsibleTrigger
                render={
                  <SidebarMenuButton
                    title={project.repoPath}
                    className={[
                      "h-8 text-[12px]",
                      // Stay highlighted while the pointer is on the row's "more" button or its
                      // menu is open. (Not group-hover: the item also holds the sub-pages.)
                      "[&:has(~[data-sidebar=menu-action]:hover)]:bg-sidebar-accent [&:has(~[data-sidebar=menu-action]:hover)]:text-sidebar-accent-foreground",
                      "[&:has(~[data-sidebar=menu-action][data-popup-open])]:bg-sidebar-accent [&:has(~[data-sidebar=menu-action][data-popup-open])]:text-sidebar-accent-foreground",
                    ].join(" ")}
                  />
                }
              >
                <span>{project.name}</span>
              </CollapsibleTrigger>
              <ProjectActionsMenu
                projectName={project.name}
                actions={[
                  {
                    id: "rename",
                    label: "Rename",
                    icon: Pencil,
                    keepsFocus: true,
                    onSelect: () => {
                      setActionError(null);
                      setRenaming(true);
                    },
                  },
                  { id: "archive", label: "Archive", icon: Archive, onSelect: () => void archive() },
                ]}
              />
            </>
          )}
          {actionError && (
            <p role="alert" className="px-3 pt-0.5 pb-1 text-[11.5px] leading-snug text-destructive">
              {actionError}
            </p>
          )}
          <CollapsibleContent
            className={[
              "h-(--collapsible-panel-height) overflow-hidden",
              "transition-[height,opacity] duration-200 ease-[cubic-bezier(0.32,0.72,0,1)]",
              "data-starting-style:h-0 data-starting-style:opacity-0 data-ending-style:h-0 data-ending-style:opacity-0",
              "motion-reduce:transition-none",
            ].join(" ")}
          >
            {/* No guide line: sub-items are set apart by their indent and dot. */}
            <SidebarMenuSub className="mx-0 translate-x-0 gap-0.5 border-l-0 px-0 py-0.5">
              {PROJECT_SECTIONS.map((section) => (
                <ProjectSectionLink
                  key={section.id}
                  to={projectSectionPath(project.id, section)}
                  section={section}
                />
              ))}
            </SidebarMenuSub>
          </CollapsibleContent>
        </Collapsible>
      </motion.div>
    </SidebarMenuItem>
  );
}
