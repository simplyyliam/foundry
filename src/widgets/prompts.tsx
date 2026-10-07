import { PromptsView } from "@/features/prompts";

export interface PromptsProps {
  /** The project from the route. */
  projectId: string;
}

/**
 * The Prompts page: hand-off and task prompt generation, and the prompt files. Keyed by
 * project, so picks and results never carry over to another project.
 */
export function Prompts({ projectId }: PromptsProps) {
  return <PromptsView key={projectId} projectId={projectId} />;
}
