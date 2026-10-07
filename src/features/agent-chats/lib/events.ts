import type { AgentEvent, Severity } from "@/lib/bindings";

export type DetailTab = "implementations" | "activity" | "danger";

export const TAB_LABELS: Record<DetailTab, string> = {
  implementations: "Implementations",
  activity: "Activity",
  danger: "Danger",
};

/** The worst severity among an agent's unread events; null when it has none. */
export type UnreadWorst = Severity | null;

/** Warnings and dangers: they go to the Danger tab, and gate marking read. */
export const isAlert = (event: Pick<AgentEvent, "severity">) => event.severity !== "info";

/** Which tab shows an event (the Danger tab reads its own stream, so only the first two matter there). */
export function tabOf(event: AgentEvent): DetailTab {
  if (isAlert(event)) return "danger";
  return event.kind === "turn_complete" ? "implementations" : "activity";
}

export const worse = (a: UnreadWorst, b: UnreadWorst): UnreadWorst => {
  const rank = (severity: UnreadWorst) => (severity === "danger" ? 3 : severity === "warn" ? 2 : severity === "info" ? 1 : 0);
  return rank(a) >= rank(b) ? a : b;
};

/** Newest first, no duplicate ids. */
export function mergeEvents(current: readonly AgentEvent[], incoming: readonly AgentEvent[]): AgentEvent[] {
  if (incoming.length === 0) return current as AgentEvent[];
  const byId = new Map(current.map((event) => [event.id, event]));
  for (const event of incoming) byId.set(event.id, event);
  return [...byId.values()].sort((a, b) => b.id - a.id);
}

const CLOCK = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
const FULL = new Intl.DateTimeFormat(undefined, { dateStyle: "full", timeStyle: "medium" });

/** "09:41" */
export const clockTime = (ms: number) => CLOCK.format(new Date(ms));
/** For tooltips: the whole date and time. */
export const fullTime = (ms: number) => FULL.format(new Date(ms));

/** "now", "3m", "2h", "5d", "8w", "1y" */
export function shortAge(ms: number, now = Date.now()): string {
  const minutes = Math.floor(Math.max(0, now - ms) / 60_000);
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  if (days < 365) return `${Math.floor(days / 7)}w`;
  return `${Math.floor(days / 365)}y`;
}
