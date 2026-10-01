import { ChevronDown } from "lucide-react";
import { useState } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { refreshProjects, useProjectsSnapshot } from "@/features/projects";
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuSkeleton,
} from "@/shared";
import { AddProjectDialog } from "./add-project/AddProjectDialog";
import { ProjectMenuItem } from "./ProjectMenuItem";

/** The "Projects" sidebar group: the add button and every active (non-archived) project. */
export function ProjectsGroup() {
  const { status, projects, error } = useProjectsSnapshot();
  const active = projects.filter((project) => !project.archived);

  // Ids present at first load; anything after that is new and animates in.
  const [baseline, setBaseline] = useState<ReadonlySet<string> | null>(null);
  if (baseline === null && status !== "loading") {
    setBaseline(new Set(projects.map((project) => project.id)));
  }
  const isNew = (id: string) => baseline !== null && !baseline.has(id);

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
            {status === "loading" ? (
              <div aria-label="Loading projects">
                <SidebarMenuSkeleton />
                <SidebarMenuSkeleton />
              </div>
            ) : status === "error" && projects.length === 0 ? (
              <div className="flex flex-col items-start gap-1 px-3 py-1.5">
                <p role="alert" className="text-[12px] leading-snug text-destructive">
                  {error?.message}
                </p>
                <button
                  type="button"
                  onClick={() => void refreshProjects()}
                  className="text-[12px] font-medium text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
                >
                  Try again
                </button>
              </div>
            ) : active.length === 0 ? (
              <p className="px-3 py-1.5 text-[12.5px] text-muted-foreground">No projects yet</p>
            ) : (
              <SidebarMenu>
                {active.map((project) => (
                  <ProjectMenuItem key={project.id} project={project} isNew={isNew(project.id)} />
                ))}
              </SidebarMenu>
            )}
          </SidebarGroupContent>
        </CollapsibleContent>
      </SidebarGroup>
    </Collapsible>
  );
}
