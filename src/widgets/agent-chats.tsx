import { AgentChats } from "@/features/agent-chats";
import { Sidebar } from "@/shared";

export interface AgentChatsPanelProps {
  /** The Teams route's `:projectId`; null asks for a project instead of showing anything. */
  projectId: string | null;
}

/** The Agent Chats view of the contextual panel (Teams). Remounted per project by the frame. */
export function AgentChatsPanel({ projectId }: AgentChatsPanelProps) {
  return (
    <Sidebar side="right" variant="inset">
      <AgentChats projectId={projectId} />
    </Sidebar>
  );
}
