import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { AgentReport, SliceReport, TeamAgent } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { usePressScale } from "@/shared";
import type { Loaded } from "../hooks/useSliceDetail";
import { agentLabel, formatWhen, plural } from "../lib/verification";
import { RequirementStatusChip } from "./Chips";
import { ErrorLine } from "./Notices";
import { Loading, Muted, Section, Subheading } from "./SheetParts";

interface ReportSectionProps {
  load: Loaded<AgentReport[]>;
  team: ReadonlyMap<string, TeamAgent>;
  requirementText: (id: string) => string | undefined;
}

/**
 * The slice's reports, each with when it was written. One report shows on its own; several get
 * a summary line and one tab per agent, opening on the newest (`writtenAt`).
 */
export function ReportSection({ load, team, requirementText }: ReportSectionProps) {
  const reports = load.status === "ready" ? load.value : [];
  return (
    <Section title={reports.length > 1 ? "Reports" : "Report"}>
      {load.status === "loading" && <Loading label="Loading the report" />}
      {load.status === "error" && <ErrorLine message={load.error.message} />}
      {load.status === "ready" && reports.length === 0 && (
        <Muted>No report yet. The agent writes one when it finishes the slice.</Muted>
      )}
      {reports.length === 1 && (
        <ReportBody report={reports[0].report} writtenAt={reports[0].writtenAt} requirementText={requirementText} />
      )}
      {reports.length > 1 && <AgentReports reports={reports} team={team} requirementText={requirementText} />}
    </Section>
  );
}

/** Milliseconds since the epoch, or null when the backend's time doesn't parse. */
const writtenMs = (writtenAt: string): number | null => {
  const ms = Date.parse(writtenAt);
  return Number.isNaN(ms) ? null : ms;
};

/** The newest report's path by `writtenAt`. Ties (and unparsable times) go to the later listed. */
function newestPath(reports: readonly AgentReport[]): string {
  let newest = reports[0];
  for (const entry of reports.slice(1)) {
    if ((writtenMs(entry.writtenAt) ?? -Infinity) >= (writtenMs(newest.writtenAt) ?? -Infinity)) newest = entry;
  }
  return newest.path;
}

function AgentReports({ reports, team, requirementText }: Omit<ReportSectionProps, "load"> & { reports: AgentReport[] }) {
  // A tab picked by hand stays picked while its report exists; otherwise follow the newest, so a
  // report written while the sheet is open (re-read on `report_ready`) comes to the front.
  const [picked, setPicked] = useState<string | null>(null);
  const selected = picked && reports.some((entry) => entry.path === picked) ? picked : newestPath(reports);

  return (
    <div className="flex flex-col gap-3">
      <p className="text-[12.5px] leading-snug text-muted-foreground">
        {plural(reports.length, "report")}:{" "}
        {reports.map((entry, index) => (
          <span key={entry.path}>
            {index > 0 && ", "}
            <code className="font-mono text-[12px] text-foreground">{entry.agent}</code>
          </span>
        ))}
      </p>
      <Tabs value={selected} onValueChange={(value) => setPicked(value as string)} className="gap-4">
        <TabsList aria-label="Reports by agent" className="h-auto! w-full flex-wrap justify-start gap-1 rounded-xl p-1">
          {reports.map((entry) => (
            <AgentTab key={entry.path} value={entry.path} writtenAt={entry.writtenAt} {...agentLabel(team, entry.agent)} />
          ))}
        </TabsList>
        {reports.map((entry) => (
          <TabsContent key={entry.path} value={entry.path}>
            <ReportBody report={entry.report} writtenAt={entry.writtenAt} requirementText={requirementText} />
          </TabsContent>
        ))}
      </Tabs>
    </div>
  );
}

function AgentTab({ value, name, role, writtenAt }: { value: string; name: string; role: string | null; writtenAt: string }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.97 });
  return (
    <TabsTrigger
      ref={ref}
      value={value}
      className="h-auto min-w-0 flex-none flex-col items-start gap-0 rounded-lg px-3 py-1.5 text-left"
    >
      <span className="max-w-56 truncate text-[12.5px] font-medium">{name}</span>
      {role && <span className="max-w-56 truncate text-[11.5px] font-normal text-muted-foreground">{role}</span>}
      <WrittenAt writtenAt={writtenAt} className="text-[11px] font-normal text-muted-foreground" />
    </TabsTrigger>
  );
}

/** "Today, 14:05" with the full date as a tooltip; the backend's text as-is if it doesn't parse. */
function WrittenAt({ writtenAt, className }: { writtenAt: string; className?: string }) {
  const ms = writtenMs(writtenAt);
  return (
    <time dateTime={writtenAt} title={ms === null ? undefined : new Date(ms).toLocaleString()} className={cn("tabular-nums", className)}>
      {ms === null ? writtenAt : formatWhen(ms)}
    </time>
  );
}

function ReportBody({
  report,
  writtenAt,
  requirementText,
}: {
  report: SliceReport;
  writtenAt: string;
  requirementText: (id: string) => string | undefined;
}) {
  return (
    <div className="flex flex-col gap-4">
      <p className="text-[12.5px] text-muted-foreground">
        By <code className="font-mono text-[12px] text-foreground">{report.agent}</code>
        <span aria-hidden> · </span>
        <span className="sr-only">, </span>
        written <WrittenAt writtenAt={writtenAt} />
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
