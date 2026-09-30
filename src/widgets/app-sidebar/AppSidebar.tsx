import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
} from "@/shared"
import { IconButton } from "@/shared/components/IconButton"
import { Bell, ChevronDown, Plus, Settings } from "lucide-react"

export function AppSidebar() {
  return (
    <Sidebar variant="inset">
      <SidebarHeader>
        <div className="flex items-center gap-2.5 w-fullw-full p-2.5">
          <div className="w-6 h-6 rounded-sm bg-red-400"></div>
          <h1 className="font-medium text-[15px]">Foundry</h1>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <Collapsible defaultOpen className="group/collapsible">
          <SidebarGroup>
            <div className="flex items-center gap-1">
              <SidebarGroupLabel render={<CollapsibleTrigger />} className="min-w-0 flex-1 pr-1">
                Projects
                <ChevronDown className="ml-auto transition-transform group-data-open/collapsible:rotate-180" />
              </SidebarGroupLabel>
              <SidebarGroupAction className="relative top-auto right-auto">
                <Plus /> <span className="sr-only">Add Project</span>
              </SidebarGroupAction>
            </div>
            <CollapsibleContent>
              <SidebarGroupContent>
              </SidebarGroupContent>
            </CollapsibleContent>
          </SidebarGroup>
        </Collapsible>
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
            <IconButton Icon={Settings} />
            <IconButton Icon={Bell} />
          </div>
        </div>
      </SidebarFooter>
    </Sidebar>
  )
}
