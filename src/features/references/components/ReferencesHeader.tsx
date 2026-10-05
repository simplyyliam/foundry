import type { ReactNode } from "react";
import { Images, Maximize2 } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { usePressScale } from "@/shared";
import { IconButton } from "@/shared/components/IconButton";
import { useBakery } from "../hooks/useBakery";
import { NEEDS_NOTE_TOOLTIP } from "../lib/reference";

interface ReferencesHeaderProps {
  projectId: string;
  /** Replaces the plain "References" title (the panel passes its view switcher). */
  title?: ReactNode;
  /** Shows the pop-out button. */
  onPopOut?: () => void;
  /** Extra actions at the end, e.g. the pop-out's close button. */
  trailing?: ReactNode;
}

/** Icon, title, pop-out, and "N need a note" while some images lack a label or a note. */
export function ReferencesHeader({ projectId, title, onPopOut, trailing }: ReferencesHeaderProps) {
  const { bakery } = useBakery(projectId);
  const popOutRef = usePressScale<HTMLButtonElement>();
  const unlabeled = bakery?.unlabeledCount ?? 0;

  return (
    <div className="flex w-full flex-col gap-0.5 px-2.5">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <span aria-hidden className="flex size-6 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
            <Images className="size-3.5" />
          </span>
          {title ?? <h1 className="truncate text-[14px] font-medium">References</h1>}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {onPopOut && (
            <Tooltip>
              <TooltipTrigger
                render={
                  <IconButton
                    ref={popOutRef}
                    Icon={Maximize2}
                    size={14}
                    ariaLabel="Open references in a larger view"
                    className="size-7 rounded-md"
                    onClick={onPopOut}
                  />
                }
              />
              <TooltipContent>Expand</TooltipContent>
            </Tooltip>
          )}
          {trailing}
        </div>
      </div>
      {/* Its own row: the 16rem panel has no room for it beside the title and the buttons. */}
      {unlabeled > 0 && (
        <div className="pl-8">
        <Tooltip>
          <TooltipTrigger
            render={
              // Focusable, so the explanation is reachable from the keyboard too.
              <span
                tabIndex={0}
                className="cursor-help rounded-md text-[12px] text-muted-foreground tabular-nums outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                {unlabeled} need{unlabeled === 1 ? "s" : ""} a note
              </span>
            }
          />
          <TooltipContent>{NEEDS_NOTE_TOOLTIP}</TooltipContent>
        </Tooltip>
        </div>
      )}
    </div>
  );
}
