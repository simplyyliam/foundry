import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { AgentReport, SliceReport, TeamAgent } from "@/lib/bindings";
import { usePressScale } from "@/shared";
import type { Loaded } from "../hooks/useSliceDetail";
import { agentLabel, plural } from "../lib/verification";
import { RequirementStatusChip } from "./Chips";
import { ErrorLine } from "./Notices";
import { Loading, Muted, Section, Subheading } from "./SheetParts";

interface ReportSectionProps {
  load: Loaded<AgentReport[]>;
  team: ReadonlyMap<string, TeamAgent>;
  /** The agent whose report was written last, when known (see `useSliceDetail`). */
  lastReporter: string | null;
  requirementText: (id: string) => string | undefined;
}

/**
 * The slice's reports. One report shows as it always did; several get a summary line and one
 * tab per agent, opening on the most recently written one when that's known.
 */
export function ReportSection({ load, team, lastReporter, requirementText }: ReportSectionProps) {
  const reports = load.status === "ready" ? load.value : [];
  return (
    <Section title={reports.length > 1 ? "Reports" : "Report"}>
      {load.status === "loading" && <Loading label="Loading the report" />}
      {load.status === "error" && <ErrorLine message={load.error.message} />}
      {load.status === "ready" && reports.length === 0 && (
        <Muted>No report yet. The agent writes one when it finishes the slice.</Muted>
      )}
      {reports.length === 1 && <ReportBody report={reports[0].report} requirementText={requirementText} />}
      {reports.length > 1 && (
        <AgentReports reports={reports} team={team} lastReporter={lastReporter} requirementText={requirementText} />
      )}
    </Section>
  );
}

/** The newest report's path: the last listed for `lastReporter` (an agent's own file comes after the legacy one). */
function newestPath(reports: readonly AgentReport[], lastReporter: string | null): string {
  const byLast = lastReporter ? reports.filter((entry) => entry.agent === lastReporter).at(-1) : undefined;
  return (byLast ?? reports[0]).path;
}

function AgentReports({ reports, team, lastReporter, requirementText }: Omit<ReportSectionProps, "load"> & { reports: AgentReport[] }) {
  // A tab picked by hand stays picked while its report exists; otherwise follow the newest.
  const [picked, setPicked] = useState<string | null>(null);
  const selected = picked && reports.some((entry) => entry.path === picked) ? picked : newestPath(reports, lastReporter);

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
            <AgentTab key={entry.path} value={entry.path} {...agentLabel(team, entry.agent)} />
          ))}
        </TabsList>
        {reports.map((entry) => (
          <TabsContent key={entry.path} value={entry.path}>
            <ReportBody report={entry.report} requirementText={requirementText} />
          </TabsContent>
        ))}
      </Tabs>
    </div>
  );
}

function AgentTab({ value, name, role }: { value: string; name: string; role: string | null }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.97 });
  return (
    <TabsTrigger
      ref={ref}
      value={value}
      className="h-auto min-w-0 flex-none flex-col items-start gap-0 rounded-lg px-3 py-1.5 text-left"
    >
      <span className="max-w-56 truncate text-[12.5px] font-medium">{name}</span>
      {role && <span className="max-w-56 truncate text-[11.5px] font-normal text-muted-foreground">{role}</span>}
    </TabsTrigger>
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
