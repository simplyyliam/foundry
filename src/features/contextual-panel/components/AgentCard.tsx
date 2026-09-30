import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { SidebarMenuButton } from "@/shared"

interface AgentCardProps {
  name: string
  role: string
  avatar: string
}


export const AgentCard = ({ name, role, avatar }: AgentCardProps) => {
  return (
    <SidebarMenuButton className="p-2.5! h-fit!">
      <Avatar size="sm">
        <AvatarImage src={avatar} />
        <AvatarFallback></AvatarFallback>
      </Avatar>
      <div className="flex items-center gap-1.5">
        <h1 className="font-medium text-[14px]">{name}</h1>
        <h1 className="font-medium text-muted-foreground text-[12px] text-nowrap">{role}</h1>
      </div>
    </SidebarMenuButton>
  )
}
