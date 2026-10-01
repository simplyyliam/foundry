import { NavLink, useMatch } from "react-router-dom";
import { cn } from "@/lib/utils";
import type { ProjectSection } from "@/features/projects";
import { SidebarMenuSubButton, SidebarMenuSubItem, usePressScale } from "@/shared";

interface ProjectSectionLinkProps {
  to: string;
  section: ProjectSection;
}

/** A project sub-item, e.g. Blueprint: a coloured dot and a label. Active on its route and below. */
export function ProjectSectionLink({ to, section }: ProjectSectionLinkProps) {
  const isActive = useMatch({ path: to, end: false }) !== null;
  const pressRef = usePressScale<HTMLAnchorElement>();

  return (
    <SidebarMenuSubItem>
      <SidebarMenuSubButton
        render={<NavLink ref={pressRef} to={to} />}
        isActive={isActive}
        className="h-8 translate-x-0 gap-3 pl-5 text-[12px] data-[size=md]:text-[12px]"
      >
        <span aria-hidden className={cn("size-2 shrink-0 rounded-full", section.dotClassName)} />
        <span>{section.label}</span>
      </SidebarMenuSubButton>
    </SidebarMenuSubItem>
  );
}
