import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  usePressScale,
  useSettingsDialog,
} from "@/shared"
import { IconButton } from "@/shared/components/IconButton"
import type { Tab } from "@/routing"
import { Bell, Settings } from "lucide-react"
import { ArchivedGroup, ProjectsGroup, SidebarTabs } from "@/features/app-sidebar"

export interface AppSidebarProps {
  /** Top-level routes shown above the groups. */
  tabs: readonly Tab[]
}

export function AppSidebar({ tabs }: AppSidebarProps) {
  const { openSettings } = useSettingsDialog()
  const settingsRef = usePressScale<HTMLButtonElement>({ scale: 0.92 })

  return (
    <Sidebar variant="inset">
      <SidebarHeader>
        <div className="flex items-center gap-2.5 w-fullw-full py-1 px-2.5">
          <div className="w-6 h-6 rounded-sm bg-red-400"></div>
          <h1 className="font-medium text-[14px]">Foundry</h1>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarTabs tabs={tabs} />
        <ArchivedGroup />
        <ProjectsGroup />
      </SidebarContent>
      <SidebarFooter>
        <div className="flex items-center justify-between gap-2.5 w-full p-2.5">
          <div className="flex items-center gap-2">
            <Avatar className="size-7!">
              <AvatarImage src="/pfp.png" />
              <AvatarFallback>S</AvatarFallback>
            </Avatar>
            <h1 className="font-medium text-[15px]">Simplyliam</h1>
          </div>
          <div className="flex items-center gap-2">
            <IconButton ref={settingsRef} Icon={Settings} ariaLabel="Settings" title="Settings" onClick={() => openSettings()} />
            <IconButton Icon={Bell} />
          </div>
        </div>
      </SidebarFooter>
    </Sidebar>
  )
}
