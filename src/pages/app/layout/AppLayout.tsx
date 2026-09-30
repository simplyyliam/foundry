import { Apptitlebar, SidebarInset, SidebarProvider, WindowFrame } from "@/shared";
import { tabs } from "@/routing";
import { AppSidebar, ContextualPanel } from "@/widgets";
import { Outlet } from "react-router-dom";

export default function AppLayout() {
  return (
    <WindowFrame titlebar={<Apptitlebar />}>
      <SidebarProvider storageKey="foundry:sidebar-open">
        <AppSidebar tabs={tabs} />
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
