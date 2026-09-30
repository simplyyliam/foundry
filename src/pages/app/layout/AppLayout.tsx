import { Apptitlebar, SidebarInset, SidebarProvider, WindowFrame } from "@/shared";
import { AppSidebar, ContextualPanel } from "@/widgets";
import { Outlet } from "react-router-dom";

export default function AppLayout() {
  return (
    <WindowFrame titlebar={<Apptitlebar />}>
      <SidebarProvider storageKey="foundry:sidebar-open">
        <AppSidebar />
        {/* Nested provider: the contextual panel has its own open state and shortcut. */}
        <SidebarProvider keyboardShortcut="mod+alt+b" storageKey="foundry:contextual-panel-open">
          <SidebarInset>
            <Outlet />
          </SidebarInset>
          <ContextualPanel />
        </SidebarProvider>
      </SidebarProvider>
    </WindowFrame>
  )
}
