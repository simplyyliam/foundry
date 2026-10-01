import { motion } from "motion/react";
import { useState } from "react";
import { useMatch } from "react-router-dom";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { PROJECT_SECTIONS, projectPath, projectSectionPath, type Project } from "@/features/projects";
import { SidebarMenuButton, SidebarMenuItem, SidebarMenuSub } from "@/shared";
import { ProjectSectionLink } from "./ProjectSectionLink";

interface ProjectMenuItemProps {
  project: Project;
  /** Added during this session: fades in instead of just appearing. */
  isNew: boolean;
}

/** A project row: clicking it shows or hides its pages (Idea, Blueprint, …). */
export function ProjectMenuItem({ project, isNew }: ProjectMenuItemProps) {
  const isInside = useMatch({ path: projectPath(project.id), end: false }) !== null;

  // Opens whenever the route enters the project; clicking the row still toggles it.
  const [open, setOpen] = useState(isInside);
  const [wasInside, setWasInside] = useState(isInside);
  if (isInside !== wasInside) {
    setWasInside(isInside);
    if (isInside) setOpen(true);
  }

  return (
    <SidebarMenuItem>
      <motion.div
        initial={isNew ? { opacity: 0, y: -4, filter: "blur(2px)" } : false}
        animate={{ opacity: 1, y: 0, filter: "blur(0px)", transitionEnd: { filter: "none" } }}
        transition={{ duration: 0.3, ease: [0.32, 0.72, 0, 1] }}
      >
        <Collapsible open={open} onOpenChange={setOpen}>
          <CollapsibleTrigger render={<SidebarMenuButton className="h-8 text-[12px]" />}>
            <span>{project.name}</span>
          </CollapsibleTrigger>
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
