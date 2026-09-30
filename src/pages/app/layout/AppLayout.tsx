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
import { AppSidebar, ContextualPanel, InsetHeader } from "@/widgets";

export default function AppLayout() {
  const { pathname } = useLocation();
  const fallbackCrumbs = useMemo(() => getRouteCrumbs(pathname), [pathname]);

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
            <ContextualPanel />
          </SidebarProvider>
        </SidebarProvider>
      </PageHeaderProvider>
    </WindowFrame>
  );
}
