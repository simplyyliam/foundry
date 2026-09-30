import { AgentCard } from "@/features/contextual-panel"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
} from "@/shared"
import { IconButton } from "@/shared/components/IconButton"
import { Plus } from "lucide-react"

export function ContextualPanel() {
  return (
    <Sidebar side="right" variant="inset">
      <SidebarHeader>
        <div className="flex items-center justify-between w-full px-2.5">
          <h1 className="font-medium text-[14px]">Agent Chats</h1>
          <div className="flex items-center gap-2">
            <IconButton Icon={Plus} className="rounded-md size-8" />
          </div>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <div className="flex flex-col p-1.5">
            <AgentCard avatar="/pfp.png" name="Mark" role="Product Lead" />
            <AgentCard avatar="/pfp.png" name="Nevaeh" role="Full-Stack Engineer" />
            <AgentCard avatar="/pfp.png" name="Lisa" role="UI/UX Designer" />
            <AgentCard avatar="/pfp.png" name="Jake" role="Manager" />
          </div>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter />
    </Sidebar>
  )
}
