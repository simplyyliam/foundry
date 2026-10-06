import type { CustomRole, HandbookRole, ProposedAgent, RosterAgent } from "@/lib/bindings";

/**
 * One card of an unconfirmed proposal. Edits stay here until Confirm; nothing is saved, so a
 * reload loses them. `clientId` is ours and never changes (names and `tempId` can).
 */
export interface DraftAgent {
  clientId: string;
  handbookRoleId: string | null;
  customRole: CustomRole | null;
  displayName: string;
  title: string;
  /** The Product Lead's reason; null for cards added by hand. */
  rationale: string | null;
  relatedSliceIds: string[];
}

let nextId = 0;
const newClientId = () => `draft-${++nextId}`;

export function draftFromProposal(agent: ProposedAgent): DraftAgent {
  return {
    clientId: newClientId(),
    handbookRoleId: agent.handbookRoleId,
    customRole: agent.customRole,
    displayName: agent.displayName,
    title: agent.title,
    rationale: agent.rationale.trim() || null,
    relatedSliceIds: agent.relatedSliceIds,
  };
}

export function draftFromRole(role: HandbookRole): DraftAgent {
  return {
    clientId: newClientId(),
    handbookRoleId: role.id,
    customRole: null,
    displayName: role.title,
    title: role.title,
    rationale: null,
    relatedSliceIds: [],
  };
}

export function draftFromCustom(role: CustomRole): DraftAgent {
  return {
    clientId: newClientId(),
    handbookRoleId: null,
    customRole: role,
    displayName: role.title,
    title: role.title,
    rationale: null,
    relatedSliceIds: [],
  };
}

/** Names compare trimmed and case-insensitively, so "Ana" and "ana " count as the same. */
export const nameKey = (name: string) => name.trim().toLocaleLowerCase();

/** Client ids of cards whose display name another card also uses. */
export function duplicateNames(drafts: readonly DraftAgent[]): Set<string> {
  const byName = new Map<string, string[]>();
  for (const draft of drafts) {
    const key = nameKey(draft.displayName);
    if (!key) continue;
    byName.set(key, [...(byName.get(key) ?? []), draft.clientId]);
  }
  return new Set([...byName.values()].filter((ids) => ids.length > 1).flat());
}

/** Why Confirm is disabled, or null when the roster can be sent. */
export function confirmBlocker(drafts: readonly DraftAgent[]): string | null {
  if (drafts.length === 0) return "Add at least one agent first.";
  if (drafts.some((draft) => !draft.displayName.trim())) return "Give every agent a display name.";
  const duplicate = drafts.find((draft) => duplicateNames(drafts).has(draft.clientId));
  if (duplicate) return `More than one agent is called “${duplicate.displayName.trim()}”. Display names must be unique.`;
  return null;
}

/** One roster entry as `confirmTeam` / `addAgentManually` take it. Globs default on the backend. */
export function rosterAgent(agent: {
  handbookRoleId: string | null;
  customRole: CustomRole | null;
  displayName: string;
  title: string;
  relatedSliceIds: string[];
}): RosterAgent {
  return {
    handbookRoleId: agent.handbookRoleId,
    customRole: agent.customRole,
    displayName: agent.displayName.trim(),
    // Empty → null: the backend then uses the role's title.
    title: agent.title.trim() || null,
    relatedSliceIds: agent.relatedSliceIds,
    avatarBuiltinId: null,
    writeGlobs: null,
    protectedGlobs: null,
  };
}

export const EMPTY_CUSTOM_ROLE: CustomRole = { title: "", description: "", skillTemplate: "" };

/** Trims every field; the title is required. */
export function cleanCustomRole(role: CustomRole): CustomRole | null {
  const title = role.title.trim();
  if (!title) return null;
  return { title, description: role.description.trim(), skillTemplate: role.skillTemplate.trim() };
}
