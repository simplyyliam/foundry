import { FileClock, LoaderCircle } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";
import type { SliceDigest, VerifySkip } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useDigest } from "../hooks/useDigest";
import { useTestCommands } from "../hooks/useTestCommands";
import { useVerifyPlan } from "../hooks/useVerifyPlan";
import { useVerifyTeam } from "../hooks/useVerifyTeam";
import { useVerifyRuns } from "../hooks/verifyRuns";
import { orderSlices } from "../lib/verification";
import { NeedsALook } from "./NeedsALook";
import { ErrorLine } from "./Notices";
import { SliceList } from "./SliceList";
import { TestCommandsCard } from "./TestCommandsCard";
import { VerificationHeader } from "./VerificationHeader";
import { VerificationSheet } from "./VerificationSheet";

/**
 * The Verification page's content: the test commands, what needs a look, and every slice with
 * its report and verification. Verifying only ever starts from a click here, or from auto
 * mode (see `verifyRuns.ts`), never from opening this page.
 */
export function VerificationView({ projectId }: { projectId: string }) {
  const plan = useVerifyPlan(projectId);
  const runs = useVerifyRuns(projectId);
  const digest = useDigest(projectId, runs.finished);
  const testCommands = useTestCommands(projectId);
  const team = useVerifyTeam(projectId);
  const [openSlice, setOpenSlice] = useState<string | null>(null);
  const commandsRef = useRef<HTMLElement>(null);

  const digestMap = useMemo(
    () =>
      digest.load.status === "ready"
        ? new Map<string, SliceDigest>(digest.load.digest.slices.map((slice) => [slice.sliceId, slice]))
        : null,
    [digest.load],
  );

  const reloadCommands = testCommands.reload;
  const reviewCommands = useCallback(() => {
    // The refusal means the list changed or was never confirmed: show what's on disk now.
    void reloadCommands();
    setOpenSlice(null);
    const card = commandsRef.current;
    if (!card) return;
    const reduced =
      window.matchMedia("(prefers-reduced-motion: reduce)").matches ||
      document.documentElement.hasAttribute("data-reduce-motion");
    card.scrollIntoView({ behavior: reduced ? "auto" : "smooth", block: "start" });
    card.focus({ preventScroll: true });
  }, [reloadCommands]);

  if (plan.load.status === "loading") {
    return (
      <div className="flex justify-center py-16">
        <LoaderCircle aria-label="Loading the plan" className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (plan.load.status === "error") {
    return (
      <ErrorLine message={plan.load.error.message}>
        <PressButton size="xs" variant="outline" className="rounded-md bg-background" onClick={plan.reload}>
          Try again
        </PressButton>
      </ErrorLine>
    );
  }

  const { slices } = plan.load.plan;
  const sliceIds = new Set(slices.map((slice) => slice.id));
  const verified = digestMap
    ? [...digestMap.values()].filter((slice) => sliceIds.has(slice.sliceId) && slice.verification === "passed").length
    : 0;
  const noReports =
    digestMap !== null && slices.length > 0 && slices.every((slice) => !digestMap.get(slice.id)?.reportPresent);
  const skips = digestMap
    ? orderSlices(slices).flatMap((slice): VerifySkip[] => {
        const skip = digestMap.get(slice.id)?.skip;
        return skip ? [skip] : [];
      })
    : [];

  return (
    <div className="flex flex-col gap-8">
      <TestCommandsCard ref={commandsRef} controller={testCommands} />

      <section aria-labelledby="verification-slices-title" className="flex flex-col gap-4">
        <h2 id="verification-slices-title" className="text-[14px] font-semibold">
          Slices
        </h2>
        {slices.length > 0 && (
          <VerificationHeader verified={verified} total={slices.length} writeDigest={digest.writeFile} />
        )}

        {digest.load.status === "error" && (
          <ErrorLine message={digest.load.error.message}>
            <PressButton size="xs" variant="outline" className="rounded-md bg-background" onClick={digest.reload}>
              Try again
            </PressButton>
          </ErrorLine>
        )}
        {digest.load.status === "ready" && (
          <NeedsALook items={digest.load.digest.attention} skips={skips} slices={slices} onOpenSlice={setOpenSlice} />
        )}

        {noReports && (
          <div role="status" className="flex items-start gap-2.5 rounded-xl border border-dashed border-border px-4 py-3">
            <FileClock aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground/70" />
            <div className="flex flex-col gap-0.5">
              <p className="text-[13px] font-medium">No reports yet</p>
              <p className="text-[12.5px] leading-snug text-pretty text-muted-foreground">
                An agent writes a report when it finishes its slice. Then you can verify it here.
              </p>
            </div>
          </div>
        )}

        <SliceList
          projectId={projectId}
          slices={slices}
          digest={digestMap}
          runs={runs}
          onOpen={setOpenSlice}
          onReviewCommands={reviewCommands}
        />
      </section>

      <VerificationSheet
        projectId={projectId}
        sliceId={openSlice}
        plan={plan.load.plan}
        digest={digestMap}
        team={team}
        runs={runs}
        markDone={plan.markDone}
        onClose={() => setOpenSlice(null)}
        onReviewCommands={reviewCommands}
      />
    </div>
  );
}
