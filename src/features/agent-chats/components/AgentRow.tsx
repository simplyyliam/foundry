import { SidebarMenuButton } from "@/shared";
import type { UnreadWorst } from "../lib/events";
import { AgentAvatar } from "./AgentAvatar";

interface AgentRowProps {
  name: string;
  title: string;
  avatar: string;
  unread: UnreadWorst;
  onOpen: () => void;
}

/** One agent in the Agent Chats list: avatar (with its unread dot), name and title. */
export function AgentRow({ name, title, avatar, unread, onOpen }: AgentRowProps) {
  return (
    <SidebarMenuButton className="h-fit! p-2.5!" onClick={onOpen}>
      <AgentAvatar src={avatar} dot={unread} className="size-6" />
      <div className="flex min-w-0 items-center gap-1.5">
        <span className="shrink-0 text-[14px] font-medium">{name}</span>
        <span className="truncate text-[12px] font-medium text-muted-foreground">{title}</span>
      </div>
    </SidebarMenuButton>
  );
}
