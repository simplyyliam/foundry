import { useCallback } from "react";
import { commands, type CustomRole, type RosterAgent, type TeamWritten } from "@/lib/bindings";
import { notifyTeamChanged } from "@/shared";
import { call, type Outcome } from "./call";

export type AddAgentOutcome =
  | { ok: true; written: TeamWritten; handbookError: string | null }
  | { ok: false; error: string };

/**
 * `addAgentManually`, then (optionally) the custom role into the Role Handbook. Every mounted
 * team view refreshes through `notifyTeamChanged` once the agent exists, even if saving the
 * role then fails: the agent was added either way.
 */
export function useAddAgent() {
  return useCallback(
    async (projectId: string, agent: RosterAgent, saveRole: CustomRole | null): Promise<AddAgentOutcome> => {
      const added: Outcome<TeamWritten> = await call(() => commands.addAgentManually(projectId, agent));
      if (!added.ok) return { ok: false, error: added.error.message };
      notifyTeamChanged(projectId);
      if (!saveRole) return { ok: true, written: added.data, handbookError: null };
      const saved = await call(() =>
        commands.roleHandbookAdd({
          title: saveRole.title,
          description: saveRole.description,
          typicalWriteGlobs: [],
          typicalProtectedGlobs: [],
          skillTemplate: saveRole.skillTemplate,
        }),
      );
      return { ok: true, written: added.data, handbookError: saved.ok ? null : saved.error.message };
    },
    [],
  );
}
