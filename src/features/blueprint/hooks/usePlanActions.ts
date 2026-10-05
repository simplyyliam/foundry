import { useMemo } from "react";
import { commands, type SliceStatus } from "@/lib/bindings";
import type { PlanController } from "./usePlan";

/** The Blueprint's plan mutations, each passing the last revision seen. */
export function usePlanActions(projectId: string, mutate: PlanController["mutate"]) {
  return useMemo(
    () => ({
      addSlice: (name: string) =>
        mutate((revision) => commands.addSlice(projectId, { name, delivers: [], dependsOn: [] }, revision)),
      renameSlice: (sliceId: string, name: string) =>
        mutate((revision) => commands.updateSlice(projectId, sliceId, { name, delivers: null }, revision)),
      removeSlice: (sliceId: string) => mutate((revision) => commands.removeSlice(projectId, sliceId, revision)),
      setDependencies: (sliceId: string, dependsOn: string[]) =>
        mutate((revision) => commands.setDependencies(projectId, sliceId, dependsOn, revision)),
      /** `reason` makes it an override (recorded as a decision by the backend). */
      setStatus: (sliceId: string, status: SliceStatus, reason: string | null = null) =>
        mutate((revision) =>
          commands.setSliceStatus(projectId, sliceId, status, { override: reason !== null, reason }, revision),
        ),
      addDecision: (sliceId: string, text: string) =>
        mutate((revision) => commands.addDecision(projectId, sliceId, text, revision)),
      addOpenQuestion: (sliceId: string, text: string) =>
        mutate((revision) => commands.addOpenQuestion(projectId, sliceId, text, revision)),
      resolveOpenQuestion: (sliceId: string, questionId: string, resolution: string) =>
        mutate((revision) => commands.resolveOpenQuestion(projectId, sliceId, questionId, resolution, revision)),
    }),
    [projectId, mutate],
  );
}

export type PlanActions = ReturnType<typeof usePlanActions>;
