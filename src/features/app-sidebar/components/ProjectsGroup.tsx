import { ChevronDown } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";
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

// Revealed while the header row is hovered or focused, or the add dialog is open. Classes are
// written out in full so Tailwind can see them.
const CHEVRON_MOTION =
  "transition-[rotate,translate] duration-250 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none group-data-open/collapsible:rotate-180";

/** The "+" is w-5 plus a 4px gap: the chevron moves 24px left to make room. */
const MAKE_ROOM =
  "group-hover/header:-translate-x-6 group-focus-within/header:-translate-x-6 group-has-data-popup-open/header:-translate-x-6";

/** Centered on the chevron's resting spot: label pr-1 (as Archived's), 16px chevron. */
const PLUS_POSITION = "absolute top-1/2 right-0.5 -translate-y-1/2";

/** Fades in out of a soft blur, drifting 4px into place. */
const PLUS_REVEAL = [
  "opacity-0 blur-[2px] translate-x-1",
  "transition-[opacity,filter,translate] duration-250 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none",
  "group-hover/header:opacity-100 group-hover/header:blur-none group-hover/header:translate-x-0",
  "group-focus-within/header:opacity-100 group-focus-within/header:blur-none group-focus-within/header:translate-x-0",
  "group-has-data-popup-open/header:opacity-100 group-has-data-popup-open/header:blur-none group-has-data-popup-open/header:translate-x-0",
].join(" ");

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
      {/* No top padding: sits tight below Archived. */}
      <SidebarGroup className="pt-0">
        {/*
          At rest the chevron sits at the far right, lined up with Archived's. Hovering (or
          focusing) this header row, and only it, slides the chevron left and fades the "+" in
          where it was. Transforms, opacity and blur only: nothing reflows.
        */}
        <div className="group/header relative">
          <SidebarGroupLabel render={<CollapsibleTrigger />} className="w-full pr-1">
            Projects
            <ChevronDown className={cn("ml-auto", CHEVRON_MOTION, MAKE_ROOM)} />
          </SidebarGroupLabel>
          <AddProjectDialog triggerClassName={cn(PLUS_POSITION, PLUS_REVEAL)} />
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
