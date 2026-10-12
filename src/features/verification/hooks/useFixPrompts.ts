import { useEffect, useState } from "react";
import { commands, type AgentReport, type FixPrompt } from "@/lib/bindings";
import { useTeamChanged } from "@/shared";
import { call } from "./call";
import type { Loaded } from "./useSliceDetail";

/**
 * One fix prompt per agent for the bugs in the slice's reports (`generateFixPrompts`, read-only).
 * Read only while some report lists bugs, again whenever the reports are re-read (a QA run or
 * an owner's report landed) and on the team-changed signal (routing follows write paths). Only
 * the newest read is kept; a failed re-read keeps what's shown.
 */
export function useFixPrompts(projectId: string, sliceId: string, reports: Loaded<AgentReport[]>): Loaded<FixPrompt[]> | null {
  const list = reports.status === "ready" ? reports.value : null;
  const hasBugs = list?.some((entry) => (entry.report.bugs?.length ?? 0) > 0) ?? false;
  const [prompts, setPrompts] = useState<Loaded<FixPrompt[]>>({ status: "loading" });
  const [teamChanged, setTeamChanged] = useState(0);
  useTeamChanged(projectId, () => setTeamChanged((count) => count + 1));

  useEffect(() => {
    if (!hasBugs) return;
    let current = true;
    void call(() => commands.generateFixPrompts(projectId, sliceId)).then((outcome) => {
      if (!current) return;
      if (outcome.ok) setPrompts({ status: "ready", value: outcome.data });
      else setPrompts((previous) => (previous.status === "ready" ? previous : { status: "error", error: outcome.error }));
    });
    return () => {
      current = false;
    };
    // `list` changes identity on every re-read of the reports, which is when the bugs may differ.
  }, [projectId, sliceId, hasBugs, list, teamChanged]);

  return hasBugs ? prompts : null;
}
