import { LoaderCircle } from "lucide-react";
import { useId, useMemo, useState } from "react";
import type { AppError, TaskPromptResult } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { PromptPlan } from "../hooks/usePromptPlan";
import type { Prompts } from "../hooks/usePrompts";
import type { TeamLoad } from "../hooks/usePromptTeam";
import { decisionText, orderSlices, unfinishedFromMessage } from "../lib/prompts";
import { ForceDialog, type ForceRequest } from "./ForceDialog";
import { Written } from "./HandoffCard";
import { ErrorLine, GateError, SectionLink, WarningNotice } from "./Notices";
import { AgentPicker, SlicePicker } from "./Pickers";
import { PromptCard } from "./PromptCard";

interface TaskPromptCardProps {
  projectId: string;
  plan: PromptPlan;
  team: { load: TeamLoad; reload: () => void };
  generate: Prompts["generateTaskPrompt"];
  onOpen: (path: string) => void;
}

/** What the last generation did, kept until the next one. */
interface Done {
  result: TaskPromptResult;
  /** Forced: the decision recorded on the slice, or the message of the failed recording. */
  decision:
    | { status: "recording" }
    | { status: "recorded"; text: string; sliceName: string }
    | { status: "failed"; message: string }
    | null;
}

/**
 * Pick a slice and an agent, then "Generate task prompt" (`force: false`). Unfinished
 * dependencies open "Generate anyway?"; confirming sends `force: true` and records the
 * decision on the slice with a freshly read revision.
 */
export function TaskPromptCard({ projectId, plan, team, generate, onOpen }: TaskPromptCardProps) {
  const sliceFieldId = useId();
  const agentFieldId = useId();
  const [sliceId, setSliceId] = useState<string | null>(null);
  const [agentId, setAgentId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [done, setDone] = useState<Done | null>(null);
  const [force, setForce] = useState<ForceRequest | null>(null);

  const slices = useMemo(() => (plan.load.status === "ready" ? orderSlices(plan.load.plan.slices) : []), [plan.load]);
  const agents = team.load.status === "ready" ? team.load.agents : null;
  const noTeam = team.load.status === "ready" && agents === null;
  const slice = slices.find((candidate) => candidate.id === sliceId) ?? null;
  const agent = agents?.find((candidate) => candidate.id === agentId) ?? null;
  const ready = plan.load.status === "ready" && agents !== null;

  const run = async () => {
    if (!slice || !agent) return;
    setBusy(true);
    setError(null);
    setDone(null);
    const outcome = await generate(slice.id, agent.id, false);
    setBusy(false);
    if (outcome.ok) setDone({ result: outcome.data, decision: null });
    else if (outcome.error.code === "dependencies_not_done") {
      const current = plan.load.status === "ready" ? plan.load.plan : null;
      setForce({
        sliceId: slice.id,
        sliceName: slice.name,
        agentId: agent.id,
        names: unfinishedFromMessage(outcome.error.message, current, slice.id),
        message: outcome.error.message,
      });
    } else setError(outcome.error);
  };

  const generateAnyway = async (request: ForceRequest): Promise<string | null> => {
    const outcome = await generate(request.sliceId, request.agentId, true);
    if (!outcome.ok) return outcome.error.message;
    setForce(null);
    const names = outcome.data.unfinishedDependencies;
    // Everything got done in the meantime: nothing was overridden, so nothing to record.
    if (names.length === 0) {
      setDone({ result: outcome.data, decision: null });
      return null;
    }
    const text = decisionText(names);
    setDone({ result: outcome.data, decision: { status: "recording" } });
    const recorded = await plan.addDecision(request.sliceId, text);
    setDone({
      result: outcome.data,
      decision: recorded.ok
        ? { status: "recorded", text, sliceName: request.sliceName }
        : { status: "failed", message: recorded.error.message },
    });
    return null;
  };

  return (
    <PromptCard
      title="Task prompt"
      description="One slice's work for one agent: what to build, what to read, and how to report back."
      disabled={noTeam}
    >
      {team.load.status === "error" && (
        <ErrorLine message={team.load.error.message}>
          <PressButton size="xs" variant="outline" className="rounded-md bg-background" onClick={team.reload}>
            Try again
          </PressButton>
        </ErrorLine>
      )}
      {plan.load.status === "error" && (
        <ErrorLine message={plan.load.error.message}>
          <PressButton size="xs" variant="outline" className="rounded-md bg-background" onClick={plan.reload}>
            Try again
          </PressButton>
        </ErrorLine>
      )}
      {noTeam && (
        <div className="flex items-center gap-3 rounded-lg bg-muted/60 py-2 pr-1.5 pl-3 text-[12.5px] ring-1 ring-foreground/5">
          <p className="min-w-0 flex-1">Confirm a team on the Setup page first.</p>
          <SectionLink projectId={projectId} sectionId="setup" label="Setup" />
        </div>
      )}

      <div className="grid gap-3 sm:grid-cols-2">
        <SlicePicker
          id={sliceFieldId}
          options={slices}
          value={sliceId}
          onChange={setSliceId}
          disabled={!ready || slices.length === 0}
          placeholder={
            plan.load.status === "loading" ? "Loading…" : plan.load.status === "ready" && slices.length === 0 ? "The plan has no slices" : "Choose a slice"
          }
        />
        <AgentPicker
          id={agentFieldId}
          options={agents ?? []}
          value={agentId}
          onChange={setAgentId}
          disabled={!ready || agents === null || agents.length === 0}
          placeholder={team.load.status === "loading" ? "Loading…" : agents?.length === 0 ? "The team has no agents" : "Choose an agent"}
        />
      </div>

      <div>
        <PressButton disabled={!ready || !slice || !agent || busy} onClick={() => void run()}>
          {busy && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
          Generate task prompt
        </PressButton>
      </div>

      {error && <GateError projectId={projectId} error={error} />}
      {done && (
        <div className="flex flex-col gap-2">
          <Written path={done.result.path} onOpen={() => onOpen(done.result.path)} />
          <WarningNotice warnings={done.result.warnings} />
          {done.decision?.status === "recording" && (
            <p className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground">
              <LoaderCircle aria-hidden className="size-3 animate-spin" />
              Recording the decision…
            </p>
          )}
          {done.decision?.status === "recorded" && (
            <p className="text-[12.5px] leading-snug text-muted-foreground">
              Recorded on {done.decision.sliceName}: “{done.decision.text}”
            </p>
          )}
          {done.decision?.status === "failed" && <ErrorLine message={done.decision.message} />}
        </div>
      )}

      <ForceDialog request={force} onClose={() => setForce(null)} onConfirm={generateAnyway} />
    </PromptCard>
  );
}
