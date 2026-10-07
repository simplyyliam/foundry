import { FolderOpen, Maximize2, Plus, Users, type LucideIcon } from "lucide-react";
import { motion } from "motion/react";
import { useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { TeamAgent } from "@/lib/bindings";
import { SidebarContent, SidebarGroup, SidebarHeader, useAddAgentDialog, usePressScale, useSidebar } from "@/shared";
import { IconButton } from "@/shared/components/IconButton";
import { useAgentAvatars } from "../hooks/useAgentAvatars";
import { useAgentDetail } from "../hooks/useAgentDetail";
import { useDocumentVisible } from "../hooks/useDocumentVisible";
import { useNow } from "../hooks/useNow";
import { useTeam } from "../hooks/useTeam";
import { useUnread } from "../hooks/useUnread";
import { AgentDetailView } from "./AgentDetailView";
import { AgentRow } from "./AgentRow";
import { DetailPopOut } from "./DetailPopOut";
import { ErrorLine } from "./ErrorLine";

const NO_TEAM_TIP = "Confirm a team on the Setup page first";
const SLIDE = { type: "spring", visualDuration: 0.25, bounce: 0 } as const;

/**
 * The Agent Chats view of the contextual panel: the team's agents with unread dots, and one
 * agent's events (Implementations, Activity, Danger) with a pop-out. The project always
 * comes from the route; without one, it asks for a project.
 */
export function AgentChats({ projectId }: { projectId: string | null }) {
  if (!projectId) {
    return (
      <>
        <ListHeader projectId={null} canAdd={false} />
        <Centered icon={FolderOpen} title="Pick a project">
          Its team's chats show up here.
        </Centered>
      </>
    );
  }
  return <ProjectChats projectId={projectId} />;
}

function ProjectChats({ projectId }: { projectId: string }) {
  const team = useTeam(projectId);
  const agents = team.status === "ready" ? (team.agents ?? []) : [];
  const avatarOf = useAgentAvatars(
    projectId,
    agents.map((agent) => agent.id),
  );
  const unread = useUnread(projectId);
  const [openId, setOpenId] = useState<string | null>(null);
  /** Whether the list was left for a detail at least once (it then slides back in). */
  const [visited, setVisited] = useState(false);
  const open = agents.find((agent) => agent.id === openId);

  if (open) {
    return (
      <motion.div
        key={open.id}
        className="flex min-h-0 flex-1 flex-col"
        initial={{ opacity: 0, x: 10 }}
        animate={{ opacity: 1, x: 0 }}
        transition={SLIDE}
      >
        {/* Keyed by agent, so its events, tab and pop-out start fresh. */}
        <DetailHost
          key={open.id}
          projectId={projectId}
          agent={open}
          avatar={avatarOf(open.id)}
          onBack={() => setOpenId(null)}
          onMarked={unread.refresh}
        />
      </motion.div>
    );
  }

  const confirmed = team.status === "ready" && team.agents !== null;
  return (
    <motion.div
      className="flex min-h-0 flex-1 flex-col"
      initial={visited ? { opacity: 0, x: -10 } : false}
      animate={{ opacity: 1, x: 0 }}
      transition={SLIDE}
    >
      <ListHeader projectId={projectId} canAdd={confirmed} loading={team.status === "loading"} />
      <SidebarContent>
        <SidebarGroup>
          {team.status === "error" && <ErrorLine message={team.error.message} className="mx-1.5" />}
          {team.status === "ready" && team.agents === null && (
            <Centered icon={Users} title="No team yet">
              Propose and confirm one on{" "}
              <Link
                to={`/projects/${projectId}/setup`}
                className="text-foreground underline underline-offset-3 outline-none hover:opacity-70 focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                Setup
              </Link>
              .
            </Centered>
          )}
          {confirmed && (
            <div className="flex flex-col p-1.5">
              {unread.error && <ErrorLine message={unread.error} className="mb-1.5" />}
              {agents.length === 0 && (
                <p className="px-2.5 py-6 text-center text-[12.5px] text-muted-foreground">The team has no agents yet.</p>
              )}
              {agents.map((agent) => (
                <AgentRow
                  key={agent.id}
                  name={agent.displayName}
                  title={agent.title}
                  avatar={avatarOf(agent.id)}
                  unread={unread.worst[agent.id] ?? null}
                  onOpen={() => {
                    setVisited(true);
                    setOpenId(agent.id);
                  }}
                />
              ))}
            </div>
          )}
        </SidebarGroup>
      </SidebarContent>
    </motion.div>
  );
}

/** One open agent: its events, shared by the panel detail and the pop-out. */
function DetailHost({
  projectId,
  agent,
  avatar,
  onBack,
  onMarked,
}: {
  projectId: string;
  agent: TeamAgent;
  avatar: string;
  onBack: () => void;
  onMarked: () => void;
}) {
  const [poppedOut, setPoppedOut] = useState(false);
  const { open: panelOpen } = useSidebar();
  const documentVisible = useDocumentVisible();
  // Events only count as seen while they're on screen.
  const visible = documentVisible && (panelOpen || poppedOut);
  const detail = useAgentDetail(projectId, agent.id, { visible, onMarked });
  const now = useNow();
  const popOutRef = usePressScale<HTMLButtonElement>();

  return (
    <>
      <AgentDetailView
        detail={detail}
        agent={agent}
        avatar={avatar}
        now={now}
        onBack={onBack}
        trailing={
          <Tooltip>
            <TooltipTrigger
              render={
                <IconButton
                  ref={popOutRef}
                  Icon={Maximize2}
                  size={13}
                  ariaLabel={`Open the chat with ${agent.displayName} in a larger view`}
                  className="size-6 rounded-md"
                  onClick={() => setPoppedOut(true)}
                />
              }
            />
            <TooltipContent>Expand</TooltipContent>
          </Tooltip>
        }
      />
      <DetailPopOut detail={detail} agent={agent} avatar={avatar} now={now} open={poppedOut} onOpenChange={setPoppedOut} />
    </>
  );
}

/** "Agent Chats" and the "+" that opens the Add agent dialog for this project. */
function ListHeader({ projectId, canAdd, loading = false }: { projectId: string | null; canAdd: boolean; loading?: boolean }) {
  const { openAddAgent } = useAddAgentDialog();
  const addRef = usePressScale<HTMLButtonElement>({ disabled: !canAdd });
  const button = (
    <IconButton
      ref={addRef}
      Icon={Plus}
      ariaLabel="Add agent"
      className="size-8 rounded-md"
      disabled={!canAdd}
      onClick={() => projectId && openAddAgent(projectId)}
    />
  );

  return (
    <SidebarHeader>
      <div className="flex w-full items-center justify-between px-2.5">
        <h1 className="text-[14px] font-medium">Agent Chats</h1>
        <div className="flex items-center gap-2">
          {loading ? (
            button
          ) : (
            <Tooltip>
              {/* A disabled button gets no pointer events, so the tooltip hangs on a wrapper. */}
              <TooltipTrigger render={<span className="inline-flex" tabIndex={canAdd ? undefined : 0} />}>{button}</TooltipTrigger>
              <TooltipContent>{canAdd ? "Add agent" : NO_TEAM_TIP}</TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>
    </SidebarHeader>
  );
}

function Centered({ icon: Icon, title, children }: { icon: LucideIcon; title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-2.5 px-6 py-12 text-center">
      <Icon aria-hidden className="size-5 text-muted-foreground" />
      <div className="flex flex-col gap-1">
        <p className="text-[13px] font-medium">{title}</p>
        <p className="text-[12.5px] leading-snug text-muted-foreground">{children}</p>
      </div>
    </div>
  );
}
