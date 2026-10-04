import { useState } from "react";
import { Roadmap, RoadmapHeader, RoadmapPopOut } from "@/features/roadmap";
import { Sidebar, SidebarContent, SidebarHeader } from "@/shared";

export interface RoadmapPanelProps {
  /** The project from the route (never a shared "active project"). */
  projectId: string;
}

/** The Roadmap view of the contextual panel, with its pop-out. */
export function RoadmapPanel({ projectId }: RoadmapPanelProps) {
  const [poppedOut, setPoppedOut] = useState(false);

  return (
    <Sidebar side="right" variant="inset">
      <SidebarHeader>
        <RoadmapHeader projectId={projectId} onPopOut={() => setPoppedOut(true)} />
      </SidebarHeader>
      <SidebarContent>
        <Roadmap projectId={projectId} ownsPrompts={!poppedOut} className="px-4.5 pt-1 pb-6" />
      </SidebarContent>
      <RoadmapPopOut projectId={projectId} open={poppedOut} onOpenChange={setPoppedOut} />
    </Sidebar>
  );
}
