import { Check, ChevronRight, CircleCheck, CircleX, History, LoaderCircle } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { CommandRun, Plan, SliceDigest, SliceReport, Verification } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { NotVerifiedDialog, PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import { useSliceDetail, type Loaded } from "../hooks/useSliceDetail";
import type { VerifyPlan } from "../hooks/useVerifyPlan";
import type { ProjectRuns } from "../hooks/verifyRuns";
import { bySeverity, formatDuration, formatWhen, plural } from "../lib/verification";
import { ReportChip, RequirementStatusChip, SeverityChip, SliceStatusChip, VerificationChip } from "./Chips";
import { ErrorLine } from "./Notices";
import { ProgressLine } from "./ProgressLine";
import { VerifyButton, VerifyError } from "./VerifyControls";

interface VerificationSheetProps {
  projectId: string;
  /** The open slice, or null when closed. */
  sliceId: string | null;
  plan: Plan;
  digest: ReadonlyMap<string, SliceDigest> | null;
  runs: ProjectRuns;
  markDone: VerifyPlan["markDone"];
  onClose: () => void;
  onReviewCommands: () => void;
}

/** One slice: its verification result and its report, with Verify and Mark done. */
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
  runs,
  markDone,
  onReviewCommands,
}: Omit<VerificationSheetProps, "sliceId" | "onClose"> & { sliceId: string }) {
  const { report, verification } = useSliceDetail(projectId, sliceId, runs.finished);
  const slice = plan.slices.find((candidate) => candidate.id === sliceId);
  const sliceDigest = digest?.get(sliceId) ?? null;
  const running = runs.running?.sliceId === sliceId ? runs.running : null;
  const done = useAction();
  const [notVerifiedOpen, setNotVerifiedOpen] = useState(false);

  // The report as read now (fresher than the digest right after a report_ready).
  const reportPresent = report.status === "ready" ? report.value !== null : (sliceDigest?.reportPresent ?? false);
  const verified = verification.status === "ready" && verification.value !== null;

  const onMarkDone = async () => {
    const outcome = await done.run(() => markDone(sliceId));
    if (!outcome.ok && outcome.error.code === "not_verified") {
      done.clear();
      setNotVerifiedOpen(true);
    }
  };

  const requirementText = (id: string) => plan.requirements.find((requirement) => requirement.id === id)?.text;

  return (
    <>
      <SheetHeader className="gap-1 pr-14 pb-4">
        <SheetTitle className="truncate text-[15px]">{slice?.name ?? sliceId}</SheetTitle>
        <SheetDescription render={<div />} className="flex flex-wrap items-center gap-1 pt-0.5">
          {slice ? <SliceStatusChip status={slice.status} /> : <span className="text-[12.5px]">This slice is no longer in the plan.</span>}
          <ReportChip present={reportPresent} />
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
        </div>
        {running && <ProgressLine projectId={projectId} run={running} className="mt-1" />}
        <VerifyError projectId={projectId} sliceId={sliceId} error={runs.errors[sliceId]} onReviewCommands={onReviewCommands} className="mt-1" />
        {done.error && <ErrorLine message={done.error.message} className="mt-1" />}
      </SheetHeader>

      <div className="min-h-0 flex-1 overflow-y-auto border-t border-border/60">
        <div className="flex flex-col gap-7 px-6 py-5">
          <VerificationSection load={verification} requirementText={requirementText} />
          <ReportSection load={report} requirementText={requirementText} />
        </div>
      </div>

      <NotVerifiedDialog
        open={notVerifiedOpen}
        onOpenChange={setNotVerifiedOpen}
        onOverride={(reason) => markDone(sliceId, reason)}
      />
    </>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5">
      <h3 className="text-[13.5px] font-semibold">{title}</h3>
      {children}
    </section>
  );
}

function Subheading({ children }: { children: ReactNode }) {
  return <h4 className="text-[12px] font-medium text-muted-foreground">{children}</h4>;
}

function Loading({ label }: { label: string }) {
  return (
    <div className="flex justify-center py-4">
      <LoaderCircle aria-label={label} className="size-4 animate-spin text-muted-foreground" />
    </div>
  );
}

function Muted({ children }: { children: ReactNode }) {
  return <p className="text-[12.5px] leading-snug text-muted-foreground">{children}</p>;
}

/* ---------- verification ---------- */

function VerificationSection({
  load,
  requirementText,
}: {
  load: Loaded<Verification | null>;
  requirementText: (id: string) => string | undefined;
}) {
  return (
    <Section title="Verification">
      {load.status === "loading" && <Loading label="Loading the verification" />}
      {load.status === "error" && <ErrorLine message={load.error.message} />}
      {load.status === "ready" && load.value === null && <Muted>Not verified yet.</Muted>}
      {load.status === "ready" && load.value !== null && <VerificationResult result={load.value} requirementText={requirementText} />}
    </Section>
  );
}

function VerificationResult({ result, requirementText }: { result: Verification; requirementText: (id: string) => string | undefined }) {
  const findings = bySeverity(result.findings);
  return (
    <div className="flex flex-col gap-4">
      <div
        role="status"
        className={cn(
          "flex items-center gap-2.5 rounded-xl px-3.5 py-2.5 text-[13px]",
          result.passed ? "bg-status-running/10" : "bg-destructive/8",
        )}
      >
        {result.passed ? (
          <CircleCheck aria-hidden className="size-4 shrink-0 text-status-running" />
        ) : (
          <CircleX aria-hidden className="size-4 shrink-0 text-destructive" />
        )}
        <span className="font-medium">{result.passed ? "Passed" : "Failed"}</span>
        <time
          dateTime={new Date(result.verifiedAt).toISOString()}
          title={new Date(result.verifiedAt).toLocaleString()}
          className="ml-auto text-[12px] text-muted-foreground tabular-nums"
        >
          {formatWhen(result.verifiedAt)}
        </time>
      </div>

      {result.stale && (
        <div role="status" className="flex items-start gap-2 rounded-lg bg-warning/12 px-3 py-2 text-[12.5px] ring-1 ring-warning/25">
          <History aria-hidden className="mt-px size-3.5 shrink-0 text-warning" />
          <p className="leading-snug">The report changed after this check. Verify again.</p>
        </div>
      )}

      <div className="flex flex-col gap-2">
        <Subheading>{findings.length === 0 ? "Findings" : `Findings (${findings.length})`}</Subheading>
        {findings.length === 0 ? (
          <Muted>No findings.</Muted>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {findings.map((finding, index) => (
              <li key={index} className="flex flex-col gap-1 rounded-lg px-3 py-2 ring-1 ring-foreground/6">
                <div className="flex flex-wrap items-center gap-1.5">
                  <SeverityChip severity={finding.severity} />
                  <code className="font-mono text-[11.5px] text-muted-foreground">{finding.code}</code>
                </div>
                <p className="text-[13px] leading-snug text-pretty">{finding.message}</p>
                {(finding.requirementId || finding.path) && (
                  <div className="flex flex-col gap-0.5 text-[12px] text-muted-foreground">
                    {finding.requirementId && (
                      <p className="min-w-0">
                        <code className="font-mono text-[11.5px]">{finding.requirementId}</code>
                        {requirementText(finding.requirementId) && (
                          <span className="ml-1.5">{requirementText(finding.requirementId)}</span>
                        )}
                      </p>
                    )}
                    {finding.path && <code className="font-mono text-[11.5px] break-all">{finding.path}</code>}
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <Subheading>Test runs</Subheading>
        {result.testRuns.length === 0 ? (
          <Muted>No test commands ran.</Muted>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {result.testRuns.map((run, index) => (
              <li key={index}>
                <TestRunRow run={run} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function TestRunRow({ run }: { run: CommandRun }) {
  const failed = run.timedOut || run.exitCode !== 0;
  return (
    <Collapsible className="rounded-lg ring-1 ring-foreground/6">
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
        <Badge variant={failed ? "destructive" : "outline"} className="h-5 px-2 font-mono text-[11px]">
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

/* ---------- report ---------- */

function ReportSection({
  load,
  requirementText,
}: {
  load: Loaded<SliceReport | null>;
  requirementText: (id: string) => string | undefined;
}) {
  return (
    <Section title="Report">
      {load.status === "loading" && <Loading label="Loading the report" />}
      {load.status === "error" && <ErrorLine message={load.error.message} />}
      {load.status === "ready" && load.value === null && (
        <Muted>No report yet. The agent writes one when it finishes the slice.</Muted>
      )}
      {load.status === "ready" && load.value !== null && <ReportBody report={load.value} requirementText={requirementText} />}
    </Section>
  );
}

function ReportBody({ report, requirementText }: { report: SliceReport; requirementText: (id: string) => string | undefined }) {
  return (
    <div className="flex flex-col gap-4">
      <p className="text-[12.5px] text-muted-foreground">
        By <code className="font-mono text-[12px] text-foreground">{report.agent}</code>
      </p>

      <div className="flex flex-col gap-2">
        <Subheading>{plural(report.requirements.length, "requirement")}</Subheading>
        {report.requirements.length > 0 && (
          <ul className="flex flex-col gap-1.5">
            {report.requirements.map((requirement, index) => (
              <li key={`${requirement.id}-${index}`} className="flex flex-col gap-1 rounded-lg px-3 py-2 ring-1 ring-foreground/6">
                <div className="flex items-start gap-2">
                  <RequirementStatusChip status={requirement.status} />
                  <p className="min-w-0 flex-1 text-[13px] leading-snug">
                    <code className="font-mono text-[11.5px] text-muted-foreground">{requirement.id}</code>
                    {requirementText(requirement.id) && <span className="ml-1.5">{requirementText(requirement.id)}</span>}
                  </p>
                </div>
                {requirement.evidence.trim() !== "" ? (
                  <p className="text-[12.5px] leading-snug whitespace-pre-wrap text-muted-foreground break-words">{requirement.evidence}</p>
                ) : (
                  <p className="text-[12.5px] text-muted-foreground italic">No evidence given.</p>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <Subheading>Tests run</Subheading>
        {report.testsRun.length === 0 ? (
          <Muted>None reported.</Muted>
        ) : (
          <ul className="flex flex-col gap-1">
            {report.testsRun.map((test, index) => (
              <li key={index} className="flex flex-col gap-0.5 text-[12.5px]">
                <code className="font-mono text-[12px] break-all">{test.command}</code>
                <span className="text-muted-foreground">{test.result}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <Subheading>Blockers</Subheading>
        {report.blockers.length === 0 ? (
          <Muted>None.</Muted>
        ) : (
          <ul className="flex list-disc flex-col gap-0.5 pl-4.5 text-[13px] leading-snug marker:text-foreground/45">
            {report.blockers.map((blocker, index) => (
              <li key={index}>{blocker}</li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <Subheading>Notes</Subheading>
        {report.notes.trim() === "" ? (
          <Muted>None.</Muted>
        ) : (
          <p className="text-[13px] leading-relaxed whitespace-pre-wrap break-words">{report.notes}</p>
        )}
      </div>
    </div>
  );
}
