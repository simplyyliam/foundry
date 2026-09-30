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
        <div className="flex items-center justify-between w-full p-2.5 px-3">
          <h1 className="font-medium text-[15px]">Agent Chats</h1>
          <div className="flex items-center gap-2">
            <IconButton Icon={Plus} className="rounded-lg" />
          </div>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <AgentCard avatar="/pfp.png" name="Mark" role="Product Lead" />
          <AgentCard avatar="/pfp.png" name="Nevaeh" role="Full-Stack Engineer" />
          <AgentCard avatar="/pfp.png" name="Lisa" role="UI/UX Designer" />
          <AgentCard avatar="/pfp.png" name="Jake" role="Manager" />
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter />
    </Sidebar>
  )
}
