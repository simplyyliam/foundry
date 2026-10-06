import type { TeamAgent, TeamWritten } from "@/lib/bindings";
import { useAgentAvatars } from "../hooks/useAgentAvatars";
import type { HandbookRoles } from "../hooks/useHandbookRoles";
import type { TeamController } from "../hooks/useTeam";
import { AddAgentButton } from "./AddAgentDialog";
import { AgentCard } from "./AgentCard";

interface ConfirmedRosterProps {
  projectId: string;
  agents: TeamAgent[];
  team: TeamController;
  handbook: HandbookRoles;
  onAdded: (written: TeamWritten) => void;
}

/** The confirmed team: one card per agent, and Add agent. The roster can't be changed otherwise. */
export function ConfirmedRoster({ projectId, agents, team, handbook, onAdded }: ConfirmedRosterProps) {
  const avatars = useAgentAvatars(
    projectId,
    agents.map((agent) => agent.id),
  );

  return (
    <div className="flex flex-col gap-4">
      {agents.length > 0 ? (
        <ul aria-label="Team" className="flex flex-col gap-2.5">
          {agents.map((agent) => (
            <AgentCard
              key={agent.id}
              agent={agent}
              avatarSrc={avatars.srcOf(agent.id)}
              roles={handbook.roles}
              onChooseAvatar={async (choice) => {
                const outcome = await team.setAvatar(agent.id, choice);
                if (!outcome.ok) return outcome.error.message;
                avatars.refresh(agent.id);
                return null;
              }}
            />
          ))}
        </ul>
      ) : (
        <p className="rounded-xl border border-dashed border-border px-4 py-6 text-center text-[12.5px] text-muted-foreground">
          The team has no agents yet.
        </p>
      )}
      <AddAgentButton handbook={handbook} addAgent={team.addAgent} onAdded={onAdded} />
    </div>
  );
}
