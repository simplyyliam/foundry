import { useState, type ReactNode } from "react";
import { References, ReferencesHeader, ReferencesPopOut } from "@/features/references";
import { Sidebar, SidebarContent, SidebarHeader } from "@/shared";

export interface ReferencesPanelProps {
  /** The project from the route (never a shared "active project"). */
  projectId: string;
  /** Replaces the plain "References" title, e.g. with the panel's view switcher. */
  title?: ReactNode;
}

/** The References view of the contextual panel (the project's Bakery images), with its pop-out. */
export function ReferencesPanel({ projectId, title }: ReferencesPanelProps) {
  const [poppedOut, setPoppedOut] = useState(false);

  return (
    <Sidebar side="right" variant="inset">
      <SidebarHeader>
        <ReferencesHeader projectId={projectId} title={title} onPopOut={() => setPoppedOut(true)} />
      </SidebarHeader>
      <SidebarContent>
        <References projectId={projectId} layout="panel" className="px-3.5 pt-1 pb-6" />
      </SidebarContent>
      <ReferencesPopOut projectId={projectId} open={poppedOut} onOpenChange={setPoppedOut} />
    </Sidebar>
  );
}
