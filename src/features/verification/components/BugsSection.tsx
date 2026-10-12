import { ChevronRight, CircleDot, OctagonAlert, TriangleAlert, type LucideIcon } from "lucide-react";
import { useId, type ReactNode } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import type { Bug, BugSeverity, FixPrompt, TeamAgent } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import type { Loaded } from "../hooks/useSliceDetail";
import { bySeverity, ownersOf, type QaRun } from "../lib/bugs";
import { agentLabel, plural } from "../lib/verification";
import { WrittenAt } from "./ReportSection";
import { Subheading } from "./SheetParts";

/** Severity is always said in words, with an icon; the colour only repeats it. */
const SEVERITY: Record<BugSeverity, { label: string; icon: LucideIcon; tone: string; iconTone: string }> = {
  high: { label: "High", icon: OctagonAlert, tone: "bg-destructive/8", iconTone: "text-destructive" },
  medium: { label: "Medium", icon: TriangleAlert, tone: "bg-warning/12", iconTone: "text-warning" },
  low: { label: "Low", icon: CircleDot, tone: "bg-muted", iconTone: "text-muted-foreground" },
};

interface BugsSectionProps {
  /** Newest first (`qaRuns`); the section isn't rendered when empty. */
  runs: QaRun[];
  /** The fix prompts, to name each bug's owner; null while there are none to read. */
  prompts: Loaded<FixPrompt[]> | null;
  team: ReadonlyMap<string, TeamAgent>;
  requirementText: (id: string) => string | undefined;
}

/**
 * The latest QA run's bugs, exactly as its report lists them now. Nothing is ever marked fixed
 * here: a bug goes away only when QA's next run stops listing it. Older runs sit behind a quiet
 * "Earlier runs" expander.
 */
export function BugsSection({ runs, prompts, team, requirementText }: BugsSectionProps) {
  const headingId = useId();
  const [latest, ...earlier] = runs;
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2.5">
      <div className="flex items-baseline gap-2">
        <h3 id={headingId} className="text-[13.5px] font-semibold">
          Bugs found
        </h3>
        <span className="text-[12px] text-muted-foreground tabular-nums">{plural(latest.bugs.length, "bug")}</span>
      </div>
      <RunByline run={latest} team={team} />
      <BugList bugs={latest.bugs} prompts={prompts} team={team} requirementText={requirementText} />
      {earlier.length > 0 && (
        <Collapsible className="flex flex-col gap-2.5">
          <CollapsibleTrigger
            render={
              <PressButton
                variant="ghost"
                pressScale={0.98}
                className="group -mx-2 h-auto w-fit justify-start gap-1.5 px-2 py-1 text-[12.5px] font-normal text-muted-foreground hover:bg-accent/60"
              />
            }
          >
            <ChevronRight aria-hidden className="size-3.5 shrink-0 transition-transform group-data-[panel-open]:rotate-90 motion-reduce:transition-none" />
            Earlier runs
            <span className="tabular-nums">({earlier.length})</span>
          </CollapsibleTrigger>
          <CollapsibleContent className="flex flex-col gap-5 border-l border-border/70 pl-3">
            {earlier.map((run) => (
              <div key={run.path} className="flex flex-col gap-2.5">
                <RunByline run={run} team={team} />
                <BugList bugs={run.bugs} prompts={prompts} team={team} requirementText={requirementText} />
              </div>
            ))}
          </CollapsibleContent>
        </Collapsible>
      )}
    </section>
  );
}

function RunByline({ run, team }: { run: QaRun; team: ReadonlyMap<string, TeamAgent> }) {
  return (
    <p className="text-[12.5px] text-muted-foreground">
      From {agentLabel(team, run.agent).name}'s run
      <span aria-hidden> · </span>
      <span className="sr-only">, </span>
      <WrittenAt writtenAt={run.writtenAt} />
    </p>
  );
}

function BugList({ bugs, ...rest }: Omit<BugsSectionProps, "runs"> & { bugs: Bug[] }) {
  return (
    <ul className="flex flex-col gap-2">
      {bySeverity(bugs).map((bug, index) => (
        <li key={`${bug.id}-${index}`}>
          <BugRow bug={bug} {...rest} />
        </li>
      ))}
    </ul>
  );
}

function BugRow({ bug, prompts, team, requirementText }: Omit<BugsSectionProps, "runs"> & { bug: Bug }) {
  const steps = bug.steps.filter((step) => step.trim() !== "");
  return (
    <article aria-label={bug.title} className="flex flex-col gap-3 rounded-lg px-3 py-2.5 ring-1 ring-foreground/6">
      <div className="flex items-start gap-2">
        <SeverityTag severity={bug.severity} />
        <p className="min-w-0 flex-1 text-[13px] leading-snug font-medium text-pretty">{bug.title}</p>
        <code className="mt-px shrink-0 font-mono text-[11.5px] text-muted-foreground">{bug.id}</code>
      </div>

      <Field label="Requirements">
        {bug.requirementIds.length === 0 ? (
          <p className="text-[12.5px] text-muted-foreground">None named.</p>
        ) : (
          <ul className="flex flex-col gap-0.5">
            {bug.requirementIds.map((id) => (
              <li key={id} className="text-[12.5px] leading-snug">
                <code className="font-mono text-[11.5px] text-muted-foreground">{id}</code>
                {requirementText(id) && <span className="ml-1.5">{requirementText(id)}</span>}
              </li>
            ))}
          </ul>
        )}
      </Field>

      <Field label="Steps">
        {steps.length === 0 ? (
          <p className="text-[12.5px] text-muted-foreground">None given.</p>
        ) : (
          <ol className="flex list-decimal flex-col gap-0.5 pl-4.5 text-[12.5px] leading-snug marker:text-muted-foreground">
            {steps.map((step, index) => (
              <li key={index}>{step}</li>
            ))}
          </ol>
        )}
      </Field>

      <Field label="Failing tests">
        {bug.failingTests.length === 0 ? (
          <p className="text-[12.5px] text-muted-foreground">None named.</p>
        ) : (
          <ul className="flex flex-col gap-1">
            {bug.failingTests.map((test, index) => (
              <li key={index} className="flex flex-col text-[12.5px] leading-snug">
                <span>{test.name}</span>
                <code className="font-mono text-[11.5px] break-all text-muted-foreground">{test.file}</code>
              </li>
            ))}
          </ul>
        )}
      </Field>

      <Field label="Owner">
        <Owner bug={bug} prompts={prompts} team={team} />
      </Field>
    </article>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1">
      <Subheading>{label}</Subheading>
      {children}
    </div>
  );
}

function SeverityTag({ severity }: { severity: BugSeverity }) {
  const { label, icon: Icon, tone, iconTone } = SEVERITY[severity];
  return (
    <span className={cn("inline-flex h-5 shrink-0 items-center gap-1 rounded-md px-1.5 text-[11.5px] font-medium", tone)}>
      <Icon aria-hidden className={cn("size-3", iconTone)} />
      {label}
      <span className="sr-only"> severity</span>
    </span>
  );
}

/** The agents the backend routes the bug to; until it answers, nothing (or the report's hint if it can't). */
function Owner({ bug, prompts, team }: { bug: Bug; prompts: Loaded<FixPrompt[]> | null; team: ReadonlyMap<string, TeamAgent> }) {
  if (prompts?.status === "ready") {
    const owners = ownersOf(prompts.value, bug.id);
    if (owners.length > 0) return <p className="text-[12.5px]">{owners.map((owner) => owner.agentName).join(", ")}</p>;
  }
  if (prompts?.status === "loading") return <p className="text-[12.5px] text-muted-foreground">Working it out…</p>;
  const hint = bug.ownerHint?.trim();
  if (hint) return <p className="text-[12.5px]">{agentLabel(team, hint).name} <span className="text-muted-foreground">(the report's suggestion)</span></p>;
  return <p className="text-[12.5px] text-muted-foreground">Unknown.</p>;
}
