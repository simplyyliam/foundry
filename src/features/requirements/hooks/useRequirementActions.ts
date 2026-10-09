import { useMemo } from "react";
import { commands, type NewRequirement, type RequirementPatch } from "@/lib/bindings";
import type { PlanController } from "./usePlan";

/** The Requirements page's plan mutations, each passing the last revision seen. */
export function useRequirementActions(projectId: string, mutate: PlanController["mutate"]) {
  return useMemo(
    () => ({
      add: (requirement: NewRequirement) =>
        mutate((revision) => commands.addRequirement(projectId, requirement, revision)),
      update: (requirementId: string, patch: RequirementPatch) =>
        mutate((revision) => commands.updateRequirement(projectId, requirementId, patch, revision)),
      /** `sliceId: null` unassigns (which also makes it proposed). */
      assign: (requirementId: string, sliceId: string | null) =>
        mutate((revision) => commands.assignRequirement(projectId, requirementId, sliceId, revision)),
      /** Constraints only. On clears the slice; off leaves it unassigned. The state is kept. */
      setProjectWide: (requirementId: string, value: boolean) =>
        mutate((revision) => commands.setRequirementProjectWide(projectId, requirementId, value, revision)),
      remove: (requirementId: string) =>
        mutate((revision) => commands.removeRequirement(projectId, requirementId, revision)),
      confirm: (requirementId: string) =>
        mutate((revision) => commands.confirmRequirement(projectId, requirementId, revision)),
      unconfirm: (requirementId: string) =>
        mutate((revision) => commands.unconfirmRequirement(projectId, requirementId, revision)),
      confirmAll: () => mutate((revision) => commands.confirmAll(projectId, revision)),
    }),
    [projectId, mutate],
  );
}

export type RequirementActions = ReturnType<typeof useRequirementActions>;
