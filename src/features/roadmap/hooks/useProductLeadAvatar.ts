import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { commands, type HandbookRole, type TeamAgent } from "@/lib/bindings";
import { useProject } from "@/features/projects";
import { call } from "./call";

/** Shown when there's no team, no Product Lead, an error, or an avatar that can't be loaded. */
export const DEFAULT_AVATAR = "/pfp.png";

const PRODUCT_LEAD = /^product lead$/i;

/** The Product Lead: by handbook role first, then by title. */
function findProductLead(agents: readonly TeamAgent[], roles: readonly HandbookRole[]) {
  const roleIds = new Set(roles.filter((role) => PRODUCT_LEAD.test(role.title.trim())).map((role) => role.id));
  return (
    agents.find((agent) => agent.sourceHandbookRoleId !== null && roleIds.has(agent.sourceHandbookRoleId)) ??
    agents.find((agent) => PRODUCT_LEAD.test(agent.title.trim()))
  );
}

function joinRepoPath(repoPath: string, relative: string) {
  const separator = repoPath.includes("\\") ? "\\" : "/";
  return `${repoPath.replace(/[\\/]+$/, "")}${separator}${relative.replace(/[\\/]+/g, separator)}`;
}

/**
 * Image URL for the confirmed team's Product Lead, or the default avatar. The caller renders
 * the default again as the image's fallback, so a URL that fails to load also lands on it.
 * Built-in avatars have no image source yet, so they use the default too.
 */
export function useProductLeadAvatar(projectId: string): string {
  const { project } = useProject(projectId);
  const repoPath = project?.repoPath;
  const [src, setSrc] = useState<{ projectId: string; url: string } | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.all([call(() => commands.getTeam(projectId)), call(() => commands.roleHandbookList())]).then(
      ([team, roles]) => {
        if (cancelled) return;
        const lead = team.ok && team.data ? findProductLead(team.data.agents, roles.ok ? roles.data : []) : undefined;
        const path = lead?.avatarPath;
        const url =
          path && !path.startsWith("builtin:") && repoPath ? convertFileSrc(joinRepoPath(repoPath, path)) : DEFAULT_AVATAR;
        setSrc({ projectId, url });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [projectId, repoPath]);

  return src?.projectId === projectId ? src.url : DEFAULT_AVATAR;
}
