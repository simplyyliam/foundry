import { Check, ChevronRight, LoaderCircle } from "lucide-react";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { CommandRun, Plan, SliceDigest, TeamAgent, Verification, VerificationGuidance } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import {
  CommitSliceButton,
  CommitSliceDialog,
  CommitSlicePrompt,
  NotVerifiedDialog,
  PressButton,
  commitAvailability,
} from "@/shared";
import { useAction } from "../hooks/useAction";
import { useAgentSend } from "../hooks/useAgentSend";
import { useCommitSetup } from "../hooks/useCommitSetup";
import { useFixPrompts } from "../hooks/useFixPrompts";
import { useGuidance } from "../hooks/useGuidance";
import { useSliceCommit } from "../hooks/useSliceCommit";
import { useSliceDetail, type Loaded } from "../hooks/useSliceDetail";
import type { VerifyPlan } from "../hooks/useVerifyPlan";
import type { ProjectRuns } from "../hooks/verifyRuns";
import { qaRuns } from "../lib/bugs";
import { formatDuration, plural } from "../lib/verification";
import { BugsSection } from "./BugsSection";
import { ReportChip, ReportCountChip, SliceStatusChip, VerificationChip } from "./Chips";
import { GuidanceCard } from "./GuidanceCard";
import { GuidedFindings } from "./GuidedFindings";
import { FixPromptsSection } from "./FixPromptsSection";
import { ErrorLine } from "./Notices";
import { ProgressLine } from "./ProgressLine";
import { ReportSection } from "./ReportSection";
import { Loading, Muted } from "./SheetParts";
import { SkipNotice } from "./SkipNotice";
import { VerifyButton, VerifyError } from "./VerifyControls";

interface VerificationSheetProps {
  projectId: string;
  /** The open slice, or null when closed. */
  sliceId: string | null;
  plan: Plan;
  digest: ReadonlyMap<string, SliceDigest> | null;
  /** The confirmed team by agent id, to name report and finding agents (ids until it loads). */
  team: ReadonlyMap<string, TeamAgent>;
  runs: ProjectRuns;
  markDone: VerifyPlan["markDone"];
  onClose: () => void;
  onReviewCommands: () => void;
}

/** One slice: its verification result and its reports, with Verify and Mark done. */
export function VerificationSheet({ sliceId, onClose, ...rest }: VerificationSheetProps) {
  // Keep showing the last slice while the sheet animates out.
  const [shown, setShown] = useState(sliceId);
  if (sliceId !== null && sliceId !== shown) setShown(sliceId);

  return (
    <Sheet open={sliceId !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full gap-0 data-[side=right]:sm:max-w-xl">
        {shown && <SliceDetails key={shown} sliceId={shown} {...rest} />}
      </SheetContent>
    </Sheet>
  );
}

function SliceDetails({
  projectId,
  sliceId,
  plan,
  digest,
  team,
  runs,
  markDone,
  onReviewCommands,
}: Omit<VerificationSheetProps, "sliceId" | "onClose"> & { sliceId: string }) {
  const { reports, verification } = useSliceDetail(projectId, sliceId, runs.finished);
  const guidance = useGuidance(projectId, sliceId, runs.finished);
  const fixPrompts = useFixPrompts(projectId, sliceId, reports);
  const agents = useAgentSend(projectId);
  const commitSetup = useCommitSetup(projectId);
  const slice = plan.slices.find((candidate) => candidate.id === sliceId);
  const sliceDigest = digest?.get(sliceId) ?? null;
  const running = runs.running?.sliceId === sliceId ? runs.running : null;
  const done = useAction();
  const [notVerifiedOpen, setNotVerifiedOpen] = useState(false);
  const commit = useSliceCommit(projectId, sliceId);
  const commitState = commitAvailability(commit);
  const [commitOpen, setCommitOpen] = useState(false);
  // Asked once, right after Mark done succeeds; it only opens the dialog.
  const [promptCommit, setPromptCommit] = useState(false);
  const openCommit = () => {
    setPromptCommit(false);
    setCommitOpen(true);
  };

  // The reports as read now (fresher than the digest right after a report_ready).
  const reportPresent = reports.status === "ready" ? reports.value.length > 0 : (sliceDigest?.reportPresent ?? false);
  const reportCount = reports.status === "ready" ? reports.value.length : (sliceDigest?.reportCount ?? 0);
  const skip = sliceDigest?.skip ?? null;
  // A refused Verify is also recorded as the skip: say it once, in plain words (with the message).
  const error = runs.errors[sliceId];
  const shownError = skip && error?.code === skip.code ? undefined : error;
  const verified = verification.status === "ready" && verification.value !== null;

  const onMarkDone = async () => {
    const outcome = await done.run(() => markDone(sliceId));
    if (outcome.ok) setPromptCommit(true);
    if (!outcome.ok && outcome.error.code === "not_verified") {
      done.clear();
      setNotVerifiedOpen(true);
    }
  };

  const guided = guidance.status === "ready" ? guidance.value : null;
  const waiting = guided?.headline.kind === "waiting";
  const overrideTemplates = guided?.actions.flatMap((action) => (action.reasonTemplate ? [action.reasonTemplate] : [])) ?? [];
  const overriding = overriddenFindings(guided);

  const bugRuns = reports.status === "ready" ? qaRuns(reports.value) : [];

  const requirementText = (id: string) => plan.requirements.find((requirement) => requirement.id === id)?.text;

  return (
    <>
      <SheetHeader className="gap-1 pr-14 pb-4">
        <SheetTitle className="truncate text-[15px]">{slice?.name ?? sliceId}</SheetTitle>
        <SheetDescription render={<div />} className="flex flex-wrap items-center gap-1 pt-0.5">
          {slice ? <SliceStatusChip status={slice.status} /> : <span className="text-[12.5px]">This slice is no longer in the plan.</span>}
          <ReportChip present={reportPresent} />
          <ReportCountChip count={reportCount} />
          {sliceDigest && <VerificationChip state={sliceDigest.verification} />}
        </SheetDescription>
        <div className="mt-2 flex flex-wrap gap-2">
          <VerifyButton projectId={projectId} sliceId={sliceId} reportPresent={reportPresent} running={running !== null} again={verified} size="sm" />
          {slice && (
            <PressButton size="sm" variant="secondary" disabled={done.busy || slice.status === "done"} onClick={() => void onMarkDone()}>
              {done.busy ? (
                <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" />
              ) : (
                <Check aria-hidden data-icon="inline-start" />
              )}
              {slice.status === "done" ? "Done" : "Mark done"}
            </PressButton>
          )}
          {slice && !(promptCommit && slice.status === "done") && <CommitSliceButton availability={commitState} onOpen={openCommit} />}
        </div>
        {slice && promptCommit && slice.status === "done" && (
          <CommitSlicePrompt availability={commitState} onOpen={openCommit} onDismiss={() => setPromptCommit(false)} className="mt-1" />
        )}
        {running && <ProgressLine projectId={projectId} run={running} className="mt-1" />}
        {skip && !running && (
          <SkipNotice projectId={projectId} skip={skip} onReviewCommands={onReviewCommands} detailed className="mt-1" />
        )}
        <VerifyError projectId={projectId} sliceId={sliceId} error={shownError} onReviewCommands={onReviewCommands} className="mt-1" />
        {done.error && <ErrorLine message={done.error.message} className="mt-1" />}
      </SheetHeader>

      <div className="min-h-0 flex-1 overflow-y-auto border-t border-border/60">
        <div className="flex flex-col gap-7 px-6 py-5">
          {guidance.status === "loading" && <Loading label="Loading what to do" />}
          {guidance.status === "error" && <ErrorLine message={guidance.error.message} />}
          {guidance.status === "ready" && (
            <GuidanceCard
              guidance={guidance.value}
              verifiedAt={verification.status === "ready" && verification.value ? verification.value.verifiedAt : null}
              projectId={projectId}
              sliceId={sliceId}
              team={team}
              reportPresent={reportPresent}
              running={running !== null}
              markDone={{ run: () => void onMarkDone(), busy: done.busy, done: slice?.status === "done" }}
              commit={{ availability: commitState, open: openCommit }}
              override={() => setNotVerifiedOpen(true)}
              commitSetup={{ ...commitSetup, run: () => void commitSetup.commit() }}
              sessions={agents.load}
              isAgentLive={agents.isAgentLive}
              send={agents.send}
            />
          )}
          {/* While agents haven't reported, everything below is provisional: shown, but muted. */}
          <div
            data-provisional={waiting || undefined}
            className={cn("flex flex-col gap-7 transition-opacity motion-reduce:transition-none", waiting && "opacity-50")}
          >
            {bugRuns.length > 0 && <BugsSection runs={bugRuns} prompts={fixPrompts} team={team} requirementText={requirementText} />}
            {fixPrompts && (
              <FixPromptsSection
                load={fixPrompts}
                team={team}
                sessions={agents.load}
                isAgentLive={agents.isAgentLive}
                send={agents.send}
              />
            )}
            <VerificationSection load={verification} guidance={guided} team={team} requirementText={requirementText} />
            <ReportSection load={reports} team={team} requirementText={requirementText} />
          </div>
        </div>
      </div>

      <NotVerifiedDialog
        open={notVerifiedOpen}
        onOpenChange={setNotVerifiedOpen}
        templates={overrideTemplates}
        overriding={overriding}
        onOverride={async (reason) => {
          const outcome = await markDone(sliceId, reason);
          if (outcome.ok) setPromptCommit(true);
          return outcome;
        }}
      />
      {slice && <CommitSliceDialog open={commitOpen} onOpenChange={setCommitOpen} sliceName={slice.name} controller={commit} />}
    </>
  );
}

/** What an override sets aside: the findings the guidance ties to it, else the "override" group. */
function overriddenFindings(guidance: VerificationGuidance | null): string[] {
  if (!guidance) return [];
  const tied = guidance.findings.filter((finding) => finding.actionIds.includes("override"));
  const list = tied.length > 0 ? tied : guidance.findings.filter((finding) => finding.group === "override");
  return list.map((finding) => finding.plain);
}

/* ---------- verification ---------- */

function VerificationSection({
  load,
  guidance,
  team,
  requirementText,
}: {
  load: Loaded<Verification | null>;
  guidance: VerificationGuidance | null;
  team: ReadonlyMap<string, TeamAgent>;
  requirementText: (id: string) => string | undefined;
}) {
  if (load.status === "loading") return <Loading label="Loading the verification" />;
  if (load.status === "error") return <ErrorLine message={load.error.message} />;
  // Not verified: the card says so, and there's nothing to list.
  if (load.value === null) return null;
  return (
    <>
      {guidance && <GuidedFindings findings={guidance.findings} review={load.value} team={team} requirementText={requirementText} />}
      <TestRuns runs={load.value.testRuns} />
    </>
  );
}

const failed = (run: CommandRun) => run.timedOut || run.exitCode !== 0;

/** Collapsed under "Test runs", except that a failing run opens it, with its output showing. */
function TestRuns({ runs }: { runs: readonly CommandRun[] }) {
  const failing = runs.filter(failed).length;
  return (
    <Collapsible defaultOpen={failing > 0} className="flex flex-col gap-2.5">
      <CollapsibleTrigger
        render={
          <PressButton
            variant="ghost"
            pressScale={0.98}
            className="group -mx-2 h-auto w-[calc(100%+1rem)] justify-start gap-1.5 px-2 py-1 text-left hover:bg-accent/60"
          />
        }
      >
        <ChevronRight aria-hidden className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-[panel-open]:rotate-90 motion-reduce:transition-none" />
        <span className="text-[13.5px] font-semibold">Test runs</span>
        <span className="text-[12px] font-normal text-muted-foreground tabular-nums">
          {runs.length === 0 ? "none" : failing > 0 ? `${failing} of ${runs.length} failed` : plural(runs.length, "run")}
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        {runs.length === 0 ? (
          <Muted>No test commands ran.</Muted>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {runs.map((run, index) => (
              <li key={index}>
                <TestRunRow run={run} />
              </li>
            ))}
          </ul>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}

function TestRunRow({ run }: { run: CommandRun }) {
  const isFailed = failed(run);
  return (
    <Collapsible defaultOpen={isFailed} className="rounded-lg ring-1 ring-foreground/6">
      <CollapsibleTrigger
        render={
          <PressButton
            variant="ghost"
            pressScale={0.99}
            className="group h-auto w-full justify-start gap-2 rounded-lg px-2.5 py-2 text-left font-normal hover:bg-accent/60"
          />
        }
      >
        <ChevronRight aria-hidden className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-[panel-open]:rotate-90 motion-reduce:transition-none" />
        <code className="min-w-0 flex-1 truncate font-mono text-[12px]" title={run.command}>
          {run.command}
        </code>
        {run.timedOut && (
          <Badge variant="destructive" className="h-5 px-2 text-[11px]">
            timed out
          </Badge>
        )}
        <Badge variant={isFailed ? "destructive" : "outline"} className="h-5 px-2 font-mono text-[11px]">
          {run.exitCode === null ? "no exit code" : `exit ${run.exitCode}`}
        </Badge>
        <span className="shrink-0 text-[12px] text-muted-foreground tabular-nums">{formatDuration(run.durationMs)}</span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        {run.tail.trim() === "" ? (
          <p className="border-t border-border/60 px-3 py-2 text-[12px] text-muted-foreground">No output.</p>
        ) : (
          <pre
            tabIndex={0}
            aria-label={`Output of ${run.command}`}
            className="max-h-80 overflow-auto rounded-b-lg border-t border-border/60 bg-muted/40 px-3 py-2 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap break-words outline-none select-text focus-visible:ring-2 focus-visible:ring-ring/40 focus-visible:ring-inset"
          >
            {run.tail}
          </pre>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}
