import { Maximize2, RefreshCw } from "lucide-react";
import type { ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { usePressScale } from "@/shared";
import { IconButton } from "@/shared/components/IconButton";
import { useGenerateControl } from "../hooks/useGenerateControl";
import { ProductLeadAvatar } from "./ProductLeadAvatar";

interface RoadmapHeaderProps {
  projectId: string;
  /** Shows the pop-out button. */
  onPopOut?: () => void;
  /** Extra actions after Regenerate, e.g. the pop-out's close button. */
  trailing?: ReactNode;
}

/** Product Lead avatar, title, then pop-out and Regenerate (once a roadmap exists). */
export function RoadmapHeader({ projectId, onPopOut, trailing }: RoadmapHeaderProps) {
  const control = useGenerateControl(projectId);
  const popOutRef = usePressScale<HTMLButtonElement>();
  const regenerateRef = usePressScale<HTMLButtonElement>({ disabled: !control.canGenerate });

  return (
    <div className="flex w-full items-center justify-between gap-2 px-2.5">
      <div className="flex min-w-0 items-center gap-2">
        <ProductLeadAvatar projectId={projectId} />
        <h1 className="truncate text-[14px] font-medium">Roadmap</h1>
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
                  ariaLabel="Open roadmap in a larger view"
                  className="size-7 rounded-md"
                  onClick={onPopOut}
                />
              }
            />
            <TooltipContent>Expand</TooltipContent>
          </Tooltip>
        )}
        {control.hasRoadmap && (
          <Tooltip>
            <TooltipTrigger
              render={
                <IconButton
                  ref={regenerateRef}
                  Icon={RefreshCw}
                  size={14}
                  ariaLabel="Regenerate roadmap"
                  className="size-7 rounded-md"
                  disabled={!control.canGenerate}
                  onClick={control.start}
                />
              }
            />
            <TooltipContent>Regenerate roadmap</TooltipContent>
          </Tooltip>
        )}
        {trailing}
      </div>
    </div>
  );
}
