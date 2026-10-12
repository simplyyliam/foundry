import type { FindingSeverity, SkipReason, SliceStatus, SliceView, TeamAgent, VerifySkip, VerifyStage } from "@/lib/bindings";

/** Slices by layer, then name (ids break ties so the order never flickers), like the Blueprint. */
export function orderSlices(slices: readonly SliceView[]): SliceView[] {
  return [...slices].sort((a, b) => a.layer - b.layer || a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
}

export const STATUS_LABEL: Record<SliceStatus, string> = {
  queued: "queued",
  in_progress: "in progress",
  done: "done",
};

export const STAGE_LABEL: Record<VerifyStage, string> = {
  starting: "Starting",
  checking: "Checking the report",
  running_tests: "Running tests",
  done: "Done",
  failed: "Failed",
  cancelled: "Cancelled",
};

const SEVERITY_RANK: Record<FindingSeverity, number> = { danger: 0, warn: 1, info: 2 };

/** Most severe first (danger, warn, info); equal severities keep their order. */
export function bySeverity<T extends { severity: FindingSeverity }>(items: readonly T[]): T[] {
  return [...items].sort((a, b) => SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity]);
}

/** 850 ms, 4.2 s, 1:05, 12:30. */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

/** Elapsed time while running: 0:04, 1:12. */
export function formatElapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function formatWhen(ms: number): string {
  const date = new Date(ms);
  const time = date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const today = new Date();
  if (date.toDateString() === today.toDateString()) return `Today, ${time}`;
  return `${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })}, ${time}`;
}

/** Milliseconds since the epoch of a report's `writtenAt`, or null when it doesn't parse. */
export function writtenMs(writtenAt: string): number | null {
  const ms = Date.parse(writtenAt);
  return Number.isNaN(ms) ? null : ms;
}

export const plural = (count: number, one: string, many = `${one}s`) => `${count} ${count === 1 ? one : many}`;

/** Why a wanted verification couldn't run, in plain words. `other` has no fixed words. */
const SKIP_TEXT: Record<Exclude<SkipReason, "other">, string> = {
  test_commands_unconfirmed: "Confirm your test commands to verify",
  no_report: "No report yet",
  report_incomplete: "The report couldn't be read",
  no_start_commit: "Move the slice to In progress first",
  another_verification_running: "Another verification is running",
  setup_not_committed: "Commit your setup files first",
};

/** The skip in plain words; for `other`, the backend's own message. */
export function skipText(skip: VerifySkip): string {
  return skip.reason === "other" ? skip.message : SKIP_TEXT[skip.reason];
}

/** The backend's message is worth showing too, unless it's already the text shown. */
export function skipDetail(skip: VerifySkip): string | null {
  return skip.reason === "other" ? null : skip.message;
}

export interface AgentLabel {
  /** The display name, or the id when the agent isn't in the team. */
  name: string;
  /** The agent's title; null when it isn't in the team. */
  role: string | null;
}

export function agentLabel(team: ReadonlyMap<string, TeamAgent>, agentId: string): AgentLabel {
  const agent = team.get(agentId);
  return agent ? { name: agent.displayName, role: agent.title.trim() || null } : { name: agentId, role: null };
}
