import { AgentsMdSection } from "@/features/setup";

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
