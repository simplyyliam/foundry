import { X } from "lucide-react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { usePressScale } from "@/shared";
import { IconButton } from "@/shared/components/IconButton";
import { Roadmap } from "./Roadmap";
import { RoadmapHeader } from "./RoadmapHeader";

interface RoadmapPopOutProps {
  projectId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Changes when a page asks to settle the conflicts: they scroll into view. */
  focusConflicts?: number;
}

/** The same Roadmap in a large scrollable dialog; boxes go two-up when there's room. */
export function RoadmapPopOut({ projectId, open, onOpenChange, focusConflicts }: RoadmapPopOutProps) {
  const closeRef = usePressScale<HTMLButtonElement>();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(85vh,56rem)] w-[min(64rem,calc(100%-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none"
      >
        <DialogTitle className="sr-only">Roadmap</DialogTitle>
        <div className="shrink-0 border-b border-border/60 px-3.5 py-3">
          <RoadmapHeader
            projectId={projectId}
            trailing={
              <DialogClose
                render={
                  <IconButton
                    ref={closeRef}
                    Icon={X}
                    size={15}
                    ariaLabel="Close"
                    variant="ghost"
                    className="size-7 rounded-md"
                  />
                }
              />
            }
          />
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-6 py-5">
          <Roadmap projectId={projectId} ownsPrompts focusConflicts={focusConflicts} />
        </div>
      </DialogContent>
    </Dialog>
  );
}
