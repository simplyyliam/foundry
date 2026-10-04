import { useMemo } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { getRouteCrumbs, tabs } from "@/routing";
import {
  Apptitlebar,
  PageHeaderProvider,
  SidebarInset,
  SidebarProvider,
  WindowFrame,
} from "@/shared";
import { useRoadmapProgressListener } from "@/features/roadmap";
import { AppSidebar, InsetHeader } from "@/widgets";
import { ContextualPanelFrame } from "./contextual-panel/ContextualPanelFrame";

export default function AppLayout() {
  const { pathname } = useLocation();
  const fallbackCrumbs = useMemo(() => getRouteCrumbs(pathname), [pathname]);
  // From app start, so a roadmap run is tracked whatever the panel shows.
  useRoadmapProgressListener();

  return (
    <WindowFrame titlebar={<Apptitlebar />}>
      <PageHeaderProvider>
        <SidebarProvider storageKey="foundry:sidebar-open">
          <AppSidebar tabs={tabs} />
          {/* Nested provider: the contextual panel has its own open state and shortcut. */}
          <SidebarProvider keyboardShortcut="mod+alt+b" storageKey="foundry:contextual-panel-open">
            <SidebarInset>
              <InsetHeader fallback={fallbackCrumbs} />
              <div className="min-h-0 flex-1 overflow-auto">
                <Outlet />
              </div>
            </SidebarInset>
            <ContextualPanelFrame />
          </SidebarProvider>
        </SidebarProvider>
      </PageHeaderProvider>
    </WindowFrame>
  );
}
