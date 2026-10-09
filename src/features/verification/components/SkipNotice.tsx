import { CircleSlash, ListChecks } from "lucide-react";
import type { VerifySkip } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import { skipDetail, skipText } from "../lib/verification";
import { SectionLink } from "./Notices";

interface SkipNoticeProps {
  projectId: string;
  skip: VerifySkip;
  onReviewCommands: () => void;
  /** The sheet also shows the backend's message under the plain words. */
  detailed?: boolean;
  className?: string;
}

/**
 * Why this slice's verification was skipped or can't run, in plain words. It stays until the
 * backend clears the skip (`verify:skip_cleared`), so there's nothing to dismiss.
 */
export function SkipNotice({ projectId, skip, onReviewCommands, detailed, className }: SkipNoticeProps) {
  const detail = detailed ? skipDetail(skip) : null;
  return (
    <div
      role="status"
      title={detailed ? undefined : skip.message}
      className={cn("flex items-start gap-2 rounded-lg bg-warning/10 py-1.5 pr-1.5 pl-3 text-[12.5px] ring-1 ring-warning/20", className)}
    >
      <CircleSlash aria-hidden className="mt-[3px] size-3.5 shrink-0 text-warning" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 py-px">
        <p className="leading-snug">
          <span className="sr-only">Not verified: </span>
          {skipText(skip)}
        </p>
        {detail && <p className="leading-snug break-words text-muted-foreground">{detail}</p>}
      </div>
      {skip.reason === "test_commands_unconfirmed" && (
        <PressButton size="xs" variant="outline" className="shrink-0 rounded-md bg-background" onClick={onReviewCommands}>
          <ListChecks aria-hidden data-icon="inline-start" />
          Review test commands
        </PressButton>
      )}
      {skip.reason === "no_start_commit" && <SectionLink projectId={projectId} sectionId="blueprint" label="Blueprint" />}
    </div>
  );
}
