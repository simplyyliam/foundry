import { generate, isBusy, useGeneration } from "./useGeneration";
import { useClaudeStatus } from "./useClaudeStatus";
import { useRoadmap } from "./useRoadmap";

/** Whether Generate / Regenerate can be clicked right now, and the action itself. */
export function useGenerateControl(projectId: string) {
  const roadmap = useRoadmap(projectId);
  const generation = useGeneration(projectId);
  const claude = useClaudeStatus();

  const busy = isBusy(generation);
  const canGenerate = claude.checked && claude.problem === null && !busy && roadmap.briefEmpty === false;

  return {
    hasRoadmap: roadmap.result !== null,
    busy,
    canGenerate,
    start: () => void generate(projectId),
  };
}
