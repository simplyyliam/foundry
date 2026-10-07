import { commands, type AgentEventCounts } from "@/lib/bindings";
import { isAlert, type UnreadWorst } from "../lib/events";
import { call, type Outcome } from "./call";

const PAGE = 500;

/**
 * The worst severity among an agent's unread events. `agentEventCounts` only tells unread
 * and danger apart, so with unread events and no danger the newest `unread` events (the
 * unread ones: everything after the read marker, newest first) are read to look for a warning.
 */
export async function unreadWorstOf(
  projectId: string,
  counts: AgentEventCounts | undefined,
): Promise<Outcome<UnreadWorst>> {
  if (!counts || counts.unread === 0) return { ok: true, data: null };
  if (counts.danger > 0) return { ok: true, data: "danger" };
  let remaining = counts.unread;
  let beforeId: number | null = null;
  while (remaining > 0) {
    const limit = Math.min(remaining, PAGE);
    const page = await call(() => commands.agentEvents(projectId, counts.agentId, { beforeId, limit, minSeverity: null }));
    if (!page.ok) return page;
    if (page.data.some(isAlert)) return { ok: true, data: "warn" };
    if (page.data.length < limit) break;
    remaining -= page.data.length;
    beforeId = page.data[page.data.length - 1].id;
  }
  return { ok: true, data: "info" };
}

/** One agent's worst unread severity, read fresh (the detail decides its tab and marking from it). */
export async function readUnreadWorst(projectId: string, agentId: string): Promise<Outcome<UnreadWorst>> {
  const counts = await call(() => commands.agentEventCounts(projectId));
  if (!counts.ok) return counts;
  return unreadWorstOf(
    projectId,
    counts.data.find((entry) => entry.agentId === agentId),
  );
}
