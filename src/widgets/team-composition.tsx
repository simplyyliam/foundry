import { AddAgentDialog as AddAgentDialogBody, TeamSection } from "@/features/team-composition";

export interface TeamCompositionProps {
  /** The project from the route. */
  projectId: string;
}

/**
 * The Setup page's Team section: propose, edit and confirm a team, or the confirmed roster.
 * Keyed by project, so an unconfirmed proposal never carries over to another project.
 */
export function TeamComposition({ projectId }: TeamCompositionProps) {
  return <TeamSection key={projectId} projectId={projectId} />;
}

/**
 * The app-wide Add agent dialog, mounted once in the app layout and opened with
 * `useAddAgentDialog().openAddAgent(projectId)` (Setup's Team section, Agent Chats' "+").
 */
export function AddAgentDialog() {
  return <AddAgentDialogBody />;
}
