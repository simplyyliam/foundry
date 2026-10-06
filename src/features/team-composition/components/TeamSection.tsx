import { LoaderCircle } from "lucide-react";
import { useState } from "react";
import type { TeamWritten } from "@/lib/bindings";
import { PressButton, SetupSection } from "@/shared";
import { useHandbookRoles } from "../hooks/useHandbookRoles";
import { usePlanStatus } from "../hooks/usePlanStatus";
import { useTeam } from "../hooks/useTeam";
import { ConfirmedRoster } from "./ConfirmedRoster";
import { ErrorLine } from "./ErrorLine";
import { ProposalFlow } from "./ProposalFlow";
import { RestoredBox } from "./RestoredBox";

const PROPOSE_INTRO =
  "The Product Lead reads your plan and proposes who's needed from your Role Handbook. Nothing is created until you confirm.";
const CONFIRMED_INTRO = "The team is confirmed. You can add agents; changing the roster is not supported yet.";

/**
 * The Team section of the Setup page. `getTeam` decides the state: no team → propose, edit
 * and confirm; a team → the confirmed roster and Add agent.
 */
export function TeamSection({ projectId }: { projectId: string }) {
  const team = useTeam(projectId);
  const plan = usePlanStatus(projectId);
  const handbook = useHandbookRoles();
  /** The last confirm/add result's `floorRestored`, while it has any. */
  const [restored, setRestored] = useState<TeamWritten["floorRestored"]>([]);
  const { load } = team;

  const onWritten = (written: TeamWritten) => setRestored(written.floorRestored);

  if (load.status !== "ready") {
    return (
      <SetupSection title="Team" intro={PROPOSE_INTRO}>
        {load.status === "loading" ? (
          <div className="flex justify-center py-10">
            <LoaderCircle aria-label="Loading the team" className="size-4 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <ErrorLine message={load.error.message}>
            <PressButton size="xs" variant="outline" className="rounded-md" onClick={() => void team.reload()}>
              Try again
            </PressButton>
          </ErrorLine>
        )}
      </SetupSection>
    );
  }

  if (load.agents === null) {
    return (
      <SetupSection title="Team" intro={PROPOSE_INTRO}>
        <ProposalFlow team={team} plan={plan} handbook={handbook} onConfirmed={onWritten} />
      </SetupSection>
    );
  }

  return (
    <SetupSection title="Team" intro={CONFIRMED_INTRO}>
      {restored.length > 0 && <RestoredBox restored={restored} agents={load.agents} />}
      <ConfirmedRoster projectId={projectId} agents={load.agents} team={team} handbook={handbook} onAdded={onWritten} />
    </SetupSection>
  );
}
