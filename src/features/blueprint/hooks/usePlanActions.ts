import { useMemo } from "react";
import { commands, type AppError, type SliceStatus } from "@/lib/bindings";
import type { PlanController } from "./usePlan";

/**
 * How a move of a "Later" item ended. There's no move command, so it's an add at the new
 * slice, then a remove of the old item: `removeFailed` means both exist now.
 */
export type MoveOutcome =
  | { ok: true; newId: string }
  | { ok: false; stage: "add"; error: AppError }
  | { ok: false; stage: "remove"; error: AppError; newId: string };

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
      addDeferred: (label: string, anchoredTo: string) =>
        mutate((revision) => commands.addDeferred(projectId, { label, anchoredTo }, revision)),
      removeDeferred: (deferredId: string) => mutate((revision) => commands.removeDeferred(projectId, deferredId, revision)),
      /**
       * Not atomic: adds a copy at the new slice, then removes the old item, each through the
       * queue with the revision current when it runs. Adding first means a failure can leave a
       * duplicate, never lose the item.
       */
      moveDeferred: async (deferredId: string, label: string, anchoredTo: string): Promise<MoveOutcome> => {
        const added = await mutate((revision) => commands.addDeferred(projectId, { label, anchoredTo }, revision));
        if (!added.ok) return { ok: false, stage: "add", error: added.error };
        const removed = await mutate((revision) => commands.removeDeferred(projectId, deferredId, revision));
        if (!removed.ok) return { ok: false, stage: "remove", error: removed.error, newId: added.data.id };
        return { ok: true, newId: added.data.id };
      },
    }),
    [projectId, mutate],
  );
}

export type PlanActions = ReturnType<typeof usePlanActions>;
