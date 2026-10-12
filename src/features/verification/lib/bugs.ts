import type { AgentReport, Bug, BugSeverity, FixPrompt } from "@/lib/bindings";
import { writtenMs } from "./verification";

/** One QA run: a report file that lists bugs, as it is on disk now. */
export interface QaRun {
  path: string;
  agent: string;
  writtenAt: string;
  bugs: Bug[];
}

/**
 * The slice's QA runs, newest first by `writtenAt` (ties and unparsable times go to the later
 * listed, like the report tabs). A report counts only while it lists at least one bug, so an
 * owner's own report never hides QA's bugs: they go only when QA's next run no longer lists them.
 */
export function qaRuns(reports: readonly AgentReport[]): QaRun[] {
  const runs = reports
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => (entry.report.bugs?.length ?? 0) > 0);
  runs.sort((a, b) => {
    const at = writtenMs(a.entry.writtenAt) ?? -Infinity;
    const bt = writtenMs(b.entry.writtenAt) ?? -Infinity;
    return bt - at || b.index - a.index;
  });
  return runs.map(({ entry }) => ({ path: entry.path, agent: entry.agent, writtenAt: entry.writtenAt, bugs: entry.report.bugs ?? [] }));
}

const SEVERITY_RANK: Record<BugSeverity, number> = { high: 0, medium: 1, low: 2 };

/** Most severe first; equal severities keep the report's order. */
export function bySeverity(bugs: readonly Bug[]): Bug[] {
  return [...bugs].sort((a, b) => SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity]);
}

/**
 * Who fixes a bug: the agents whose fix prompt includes its id (the backend routes by write
 * paths, then `ownerHint`, then the Product Lead; a bug spanning two owners goes to both).
 */
export function ownersOf(prompts: readonly FixPrompt[], bugId: string): FixPrompt[] {
  return prompts.filter((prompt) => prompt.bugIds.includes(bugId));
}
