import { AgentsMdSection, CommitSetupSection } from "@/features/setup";

export interface SetupProps {
  /** The project from the route. */
  projectId: string;
}

/**
 * The Setup page's AGENTS.md section. The page stacks it with the Team section (a separate
 * widget) and owns the scrolling column.
 */
export function Setup({ projectId }: SetupProps) {
  return <AgentsMdSection key={projectId} projectId={projectId} />;
}

/**
 * The Setup page's last section: "Commit setup", only while setup files need committing. The
 * page places it after Team (confirming a team writes setup files too).
 */
export function SetupCommit({ projectId }: SetupProps) {
  return <CommitSetupSection key={projectId} projectId={projectId} />;
}
