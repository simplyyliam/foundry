import { useState, type ReactNode } from "react";
import { Roadmap, RoadmapHeader, RoadmapPopOut } from "@/features/roadmap";
import { Sidebar, SidebarContent, SidebarHeader, usePanelRequest } from "@/shared";

export interface RoadmapPanelProps {
  /** The project from the route (never a shared "active project"). */
  projectId: string;
  /** Replaces the plain "Roadmap" title, e.g. with the panel's view switcher. */
  title?: ReactNode;
}

/** The Roadmap view of the contextual panel, with its pop-out. */
export function RoadmapPanel({ projectId, title }: RoadmapPanelProps) {
  const [poppedOut, setPoppedOut] = useState(false);
  const [focusConflicts, setFocusConflicts] = useState(0);
  // "Settle them" (Requirements): the conflicts in the wide pop-out, where excerpts sit side by side.
  usePanelRequest("settle-conflicts", projectId, () => {
    setPoppedOut(true);
    setFocusConflicts((token) => token + 1);
  });

  return (
    <Sidebar side="right" variant="inset">
      <SidebarHeader>
        <RoadmapHeader projectId={projectId} title={title} onPopOut={() => setPoppedOut(true)} />
      </SidebarHeader>
      <SidebarContent>
        <Roadmap projectId={projectId} ownsPrompts={!poppedOut} className="px-4.5 pt-1 pb-6" />
      </SidebarContent>
      <RoadmapPopOut projectId={projectId} open={poppedOut} onOpenChange={setPoppedOut} focusConflicts={focusConflicts} />
    </Sidebar>
  );
}
