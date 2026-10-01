import { ChevronDown } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { useProjectsSnapshot } from "@/features/projects";
import { SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarMenu } from "@/shared";
import { ArchivedProjectItem } from "./ArchivedProjectItem";

/** The "Archived" sidebar group: always shown, collapsible like Projects, closed by default. */
export function ArchivedGroup() {
  const { status, projects } = useProjectsSnapshot();
  const archived = projects.filter((project) => project.archived);

  return (
    <Collapsible className="group/archived">
      {/* No bottom padding: sits tight above Projects. */}
      <SidebarGroup className="pb-0">
        <SidebarGroupLabel render={<CollapsibleTrigger />} className="min-w-0 pr-1">
          Archived
          {archived.length > 0 && (
            <Badge variant="secondary" className="ml-1.5 h-4.5 min-w-4.5 px-1.5 text-[11px] tabular-nums">
              {archived.length}
            </Badge>
          )}
          <ChevronDown className="ml-auto transition-transform group-data-open/archived:rotate-180" />
        </SidebarGroupLabel>
        <CollapsibleContent>
          <SidebarGroupContent>
            {status === "loading" ? null : archived.length === 0 ? (
              <p className="px-3 py-1.5 text-[12.5px] text-muted-foreground">No archived projects</p>
            ) : (
              <SidebarMenu>
                {archived.map((project) => (
                  <ArchivedProjectItem key={project.id} project={project} />
                ))}
              </SidebarMenu>
            )}
          </SidebarGroupContent>
        </CollapsibleContent>
      </SidebarGroup>
    </Collapsible>
  );
}
