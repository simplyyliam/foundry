import { Layers } from "lucide-react";
import type { SliceDigest, SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { ProjectRuns } from "../hooks/verifyRuns";
import { orderSlices } from "../lib/verification";
import { ReportChip, SliceStatusChip, VerificationChip } from "./Chips";
import { SectionLink } from "./Notices";
import { ProgressLine } from "./ProgressLine";
import { SkipNotice } from "./SkipNotice";
import { VerifyButton, VerifyError } from "./VerifyControls";

interface SliceListProps {
  projectId: string;
  slices: readonly SliceView[];
  /** Null until the digest has loaded: report and verification chips wait for it. */
  digest: ReadonlyMap<string, SliceDigest> | null;
  /** Reports per slice, when known (for the chip's count). */
  reportCounts: Readonly<Record<string, number>>;
  runs: ProjectRuns;
  onOpen: (sliceId: string) => void;
  onReviewCommands: () => void;
}

/** Every slice in layer order, with its report, verification and a Verify action. */
export function SliceList({ projectId, slices, digest, reportCounts, runs, onOpen, onReviewCommands }: SliceListProps) {
  if (slices.length === 0) {
    return (
      <div className="flex flex-col items-center gap-1 rounded-xl border border-dashed border-border px-6 py-8 text-center">
        <Layers aria-hidden className="mb-1 size-5 text-muted-foreground/70" />
        <p className="text-[13px] font-medium">No slices yet</p>
        <p className="mb-2 max-w-xs text-[12.5px] leading-snug text-pretty text-muted-foreground">
          Slices come from the roadmap. Each one gets verified here once its agent writes a report.
        </p>
        <SectionLink projectId={projectId} sectionId="blueprint" label="Blueprint" />
      </div>
    );
  }

  return (
    <ul className="flex flex-col gap-1.5" aria-label="Slices">
      {orderSlices(slices).map((slice) => (
        <li key={slice.id}>
          <SliceRow
            projectId={projectId}
            slice={slice}
            digest={digest?.get(slice.id) ?? null}
            reportCount={reportCounts[slice.id]}
            runs={runs}
            onOpen={() => onOpen(slice.id)}
            onReviewCommands={onReviewCommands}
          />
        </li>
      ))}
    </ul>
  );
}

interface SliceRowProps {
  projectId: string;
  slice: SliceView;
  digest: SliceDigest | null;
  reportCount: number | undefined;
  runs: ProjectRuns;
  onOpen: () => void;
  onReviewCommands: () => void;
}

function SliceRow({ projectId, slice, digest, reportCount, runs, onOpen, onReviewCommands }: SliceRowProps) {
  const running = runs.running?.sliceId === slice.id ? runs.running : null;
  const skip = digest?.skip ?? null;
  // A refused Verify is also recorded as the skip: say it once, in plain words.
  const error = runs.errors[slice.id];
  const shownError = skip && error?.code === skip.code ? undefined : error;
  return (
    <div className="flex flex-col gap-1.5 rounded-lg p-1 ring-1 ring-foreground/6">
      <div className="flex items-center gap-2">
        <PressButton
          variant="ghost"
          pressScale={0.99}
          onClick={onOpen}
          className="h-auto min-w-0 flex-1 justify-start gap-3 rounded-md px-2.5 py-2 text-left font-normal hover:bg-accent/60"
        >
          <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{slice.name}</span>
          <span className="flex shrink-0 flex-wrap items-center justify-end gap-1">
            <SliceStatusChip status={slice.status} />
            {digest && <ReportChip present={digest.reportPresent} count={reportCount} />}
            {digest && <VerificationChip state={digest.verification} />}
          </span>
        </PressButton>
        {digest && (
          <div className="shrink-0 pr-1.5">
            <VerifyButton projectId={projectId} sliceId={slice.id} reportPresent={digest.reportPresent} running={running !== null} />
          </div>
        )}
      </div>
      {running && <ProgressLine projectId={projectId} run={running} className="px-1.5 pb-1" />}
      {skip && !running && (
        <SkipNotice projectId={projectId} skip={skip} onReviewCommands={onReviewCommands} className="mx-1.5 mb-1" />
      )}
      <VerifyError
        projectId={projectId}
        sliceId={slice.id}
        error={shownError}
        onReviewCommands={onReviewCommands}
        className="mx-1.5 mb-1"
      />
    </div>
  );
}
