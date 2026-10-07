import { Users } from "lucide-react";
import { Link } from "react-router-dom";
import { useTeam } from "../hooks/useTeam";
import { ErrorLine } from "./ErrorLine";
import { TeamBoard } from "./TeamBoard";

function NoTeam({ projectId }: { projectId: string }) {
  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center gap-3 text-center">
      <Users aria-hidden className="size-6 text-muted-foreground" />
      <div className="flex flex-col gap-1">
        <p className="text-[14px] font-medium">This project has no team yet</p>
        <p className="text-[13px] text-muted-foreground">
          Propose and confirm one on{" "}
          <Link
            to={`/projects/${projectId}/setup`}
            className="text-foreground underline underline-offset-3 outline-none hover:opacity-70 focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            Setup
          </Link>
          , then come back to start its agents.
        </p>
      </div>
    </div>
  );
}

/** The Teams page body for one project: the team's terminal cards, or why there are none. */
export function TeamsView({ projectId }: { projectId: string }) {
  const team = useTeam(projectId);
  if (team.status === "loading") return null;
  if (team.status === "error") return <ErrorLine message={team.error.message} />;
  if (!team.agents || team.agents.length === 0) return <NoTeam projectId={projectId} />;
  return <TeamBoard projectId={projectId} agents={team.agents} />;
}
