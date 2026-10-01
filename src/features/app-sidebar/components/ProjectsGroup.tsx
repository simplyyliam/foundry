import { ChevronDown } from "lucide-react";
import { useState } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { useProjects } from "@/features/projects";
import { SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarMenu } from "@/shared";
import { AddProjectDialog } from "./add-project/AddProjectDialog";
import { ProjectMenuItem } from "./ProjectMenuItem";

/** The "Projects" sidebar group: its add button and one item per project. */
export function ProjectsGroup() {
  const projects = useProjects();
  // Projects created after this point animate in; ones loaded at startup don't.
  const [mountedAt] = useState(() => Date.now());

  return (
    <Collapsible defaultOpen className="group/collapsible">
      <SidebarGroup>
        <div className="flex items-center gap-1">
          <SidebarGroupLabel render={<CollapsibleTrigger />} className="min-w-0 flex-1 pr-1">
            Projects
            <ChevronDown className="ml-auto transition-transform group-data-open/collapsible:rotate-180" />
          </SidebarGroupLabel>
          <AddProjectDialog />
        </div>
        <CollapsibleContent>
          <SidebarGroupContent>
            {projects.length === 0 ? (
              <p className="px-3 py-1.5 text-[12.5px] text-muted-foreground">No projects yet</p>
            ) : (
              <SidebarMenu>
                {projects.map((project) => (
                  <ProjectMenuItem
                    key={project.id}
                    project={project}
                    isNew={project.createdAt > mountedAt}
                  />
                ))}
              </SidebarMenu>
            )}
          </SidebarGroupContent>
        </CollapsibleContent>
      </SidebarGroup>
    </Collapsible>
  );
}
