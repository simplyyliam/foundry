import { ChevronRight, UserRound } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import type { Finding, FindingGroup, GuidedFinding, RequirementOutcome, TeamAgent, Verification } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { agentLabel } from "../lib/verification";
import { RequirementStatusChip, SeverityChip } from "./Chips";
import { Muted, Section } from "./SheetParts";

/** Shown groups, in order. `info` and `ignore` go under "Show N more". */
const GROUPS: { group: FindingGroup; title: string }[] = [
  { group: "fix", title: "To fix" },
  { group: "override", title: "Fine to override" },
  { group: "decide", title: "Needs your decision" },
  { group: "follow_up", title: "Worth a follow-up" },
];
const MORE: { group: FindingGroup; title: string }[] = [
  { group: "info", title: "For information" },
  { group: "ignore", title: "Nothing to do" },
];

interface GuidedFindingsProps {
  findings: readonly GuidedFinding[];
  /** The verification the findings come from: their raw codes and details. */
  review: Verification | null;
  team: ReadonlyMap<string, TeamAgent>;
  requirementText: (id: string) => string | undefined;
}

/** The guidance's findings, grouped by what they need, each in plain words. */
export function GuidedFindings({ findings, review, team, requirementText }: GuidedFindingsProps) {
  const hidden = findings.filter((finding) => MORE.some(({ group }) => group === finding.group));
  const raw = (finding: GuidedFinding) => rawFinding(finding.findingId, review);
  const [showMore, setShowMore] = useState(false);
  return (
    <Section title={findings.length === 0 ? "Findings" : `Findings (${findings.length})`}>
      {findings.length === 0 && <Muted>No findings.</Muted>}
      {GROUPS.map(({ group, title }) => (
        <FindingGroupList
          key={group}
          title={title}
          findings={findings.filter((finding) => finding.group === group)}
          raw={raw}
          team={team}
          requirementText={requirementText}
        />
      ))}
      {hidden.length > 0 && (
        <Collapsible open={showMore} onOpenChange={setShowMore} className="flex flex-col gap-2.5">
          <CollapsibleTrigger
            render={
              <PressButton
                variant="ghost"
                size="sm"
                pressScale={0.98}
                className="group -ml-2 w-fit gap-1.5 px-2 text-[12.5px] font-medium text-muted-foreground"
              />
            }
          >
            <ChevronRight aria-hidden className="size-3.5 transition-transform group-data-[panel-open]:rotate-90 motion-reduce:transition-none" />
            {showMore ? "Show less" : `Show ${hidden.length} more`}
          </CollapsibleTrigger>
          <CollapsibleContent className="flex flex-col gap-4">
            {MORE.map(({ group, title }) => (
              <FindingGroupList
                key={group}
                title={title}
                findings={findings.filter((finding) => finding.group === group)}
                raw={raw}
                team={team}
                requirementText={requirementText}
              />
            ))}
          </CollapsibleContent>
        </Collapsible>
      )}
    </Section>
  );
}

type Raw = { kind: "finding"; finding: Finding } | { kind: "requirement"; outcome: RequirementOutcome } | null;

/** `f<index>` → the review's finding; `r:<requirementId>` → that requirement's outcome. */
function rawFinding(findingId: string, review: Verification | null): Raw {
  if (!review) return null;
  if (findingId.startsWith("r:")) {
    const outcome = review.requirements?.find((candidate) => candidate.id === findingId.slice(2));
    return outcome ? { kind: "requirement", outcome } : null;
  }
  const index = /^f(\d+)$/.exec(findingId)?.[1];
  const finding = index === undefined ? undefined : review.findings[Number(index)];
  return finding ? { kind: "finding", finding } : null;
}

function FindingGroupList({
  title,
  findings,
  raw,
  team,
  requirementText,
}: {
  title: string;
  findings: readonly GuidedFinding[];
  raw: (finding: GuidedFinding) => Raw;
  team: ReadonlyMap<string, TeamAgent>;
  requirementText: (id: string) => string | undefined;
}) {
  if (findings.length === 0) return null;
  return (
    <div role="group" aria-label={title} className="flex flex-col gap-1.5">
      <h4 className="text-[12px] font-medium text-muted-foreground">
        {title} <span className="tabular-nums">({findings.length})</span>
      </h4>
      <ul className="flex flex-col gap-1.5">
        {findings.map((finding) => (
          <li key={finding.findingId} className="flex flex-col gap-1 rounded-lg px-3 py-2 ring-1 ring-foreground/6">
            <p className="text-[13px] leading-snug text-pretty">{finding.plain}</p>
            <p className="text-[12.5px] leading-snug text-pretty text-muted-foreground">{finding.whatToDo}</p>
            <FindingDetails finding={finding} raw={raw(finding)} team={team} requirementText={requirementText} />
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The raw finding (code, message, requirement, file, agent) and any test output, on demand. */
function FindingDetails({
  finding,
  raw,
  team,
  requirementText,
}: {
  finding: GuidedFinding;
  raw: Raw;
  team: ReadonlyMap<string, TeamAgent>;
  requirementText: (id: string) => string | undefined;
}) {
  if (!raw && !finding.output) return null;
  const requirementId = raw?.kind === "finding" ? raw.finding.requirementId : raw?.kind === "requirement" ? raw.outcome.id : null;
  return (
    <Collapsible>
      <CollapsibleTrigger
        render={
          <PressButton
            variant="ghost"
            size="xs"
            pressScale={0.97}
            className="group -ml-1.5 h-6 w-fit gap-1 px-1.5 text-[11.5px] font-normal text-muted-foreground"
          />
        }
      >
        <ChevronRight aria-hidden className="size-3 transition-transform group-data-[panel-open]:rotate-90 motion-reduce:transition-none" />
        Details
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-1.5 pt-1 text-[12px] text-muted-foreground">
        {raw?.kind === "finding" && (
          <>
            <div className="flex flex-wrap items-center gap-1.5">
              <SeverityChip severity={raw.finding.severity} />
              <code className="font-mono text-[11.5px]">{raw.finding.code}</code>
              {raw.finding.agent && <AgentName agentId={raw.finding.agent} team={team} />}
            </div>
            <p className="leading-snug text-pretty text-foreground/85 select-text">{raw.finding.message}</p>
            {raw.finding.path && <code className="font-mono text-[11.5px] break-all select-text">{raw.finding.path}</code>}
          </>
        )}
        {raw?.kind === "requirement" && raw.outcome.status && (
          <div className="flex flex-wrap items-center gap-1.5">
            <RequirementStatusChip status={raw.outcome.status} />
            {raw.outcome.reportedBy.map((reported) => (
              <AgentName key={reported.agent} agentId={reported.agent} team={team} />
            ))}
          </div>
        )}
        {requirementId && (
          <Detail>
            <code className="font-mono text-[11.5px]">{requirementId}</code>
            {requirementText(requirementId) && <span className="ml-1.5">{requirementText(requirementId)}</span>}
          </Detail>
        )}
        {finding.output && (
          <pre
            tabIndex={0}
            aria-label="Test output"
            className="max-h-60 overflow-auto rounded-md bg-muted/40 px-2.5 py-2 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap break-words text-foreground outline-none select-text focus-visible:ring-2 focus-visible:ring-ring/40"
          >
            {finding.output}
          </pre>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}

function Detail({ children }: { children: ReactNode }) {
  return <p className="min-w-0 leading-snug">{children}</p>;
}

function AgentName({ agentId, team }: { agentId: string; team: ReadonlyMap<string, TeamAgent> }) {
  const { name, role } = agentLabel(team, agentId);
  return (
    <span className="flex min-w-0 items-center gap-1" title={role ? `${name}, ${role}` : name}>
      <UserRound aria-hidden className="size-3 shrink-0" />
      <span className="sr-only">From</span>
      <span className="truncate font-medium text-foreground/80">{name}</span>
    </span>
  );
}
