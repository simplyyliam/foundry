import { TeamsView } from "@/features/teams";

export interface TeamsProps {
  /** The project from the route. */
  projectId: string;
}

/**
 * The Teams page: one live terminal card per agent of the project's confirmed team. Keyed by
 * project, so terminals, selection and start dialogs never carry over to another project.
 */
export function Teams({ projectId }: TeamsProps) {
  return <TeamsView key={projectId} projectId={projectId} />;
}
