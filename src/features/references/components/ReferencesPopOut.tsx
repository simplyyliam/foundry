import { X } from "lucide-react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { usePressScale } from "@/shared";
import { IconButton } from "@/shared/components/IconButton";
import { References } from "./References";
import { ReferencesHeader } from "./ReferencesHeader";

interface ReferencesPopOutProps {
  projectId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** The same References in a large scrollable dialog, every card editable inline. */
export function ReferencesPopOut({ projectId, open, onOpenChange }: ReferencesPopOutProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(85vh,56rem)] w-[min(72rem,calc(100%-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none"
      >
        <DialogTitle className="sr-only">References</DialogTitle>
        <div className="shrink-0 border-b border-border/60 px-3.5 py-3">
          <ReferencesHeader projectId={projectId} trailing={<CloseButton />} />
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-6 py-5">
          <References projectId={projectId} layout="popout" />
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** Its own component so the press feedback attaches when the dialog mounts it. */
function CloseButton() {
  const ref = usePressScale<HTMLButtonElement>();
  return (
    <DialogClose
      render={<IconButton ref={ref} Icon={X} size={15} ariaLabel="Close" variant="ghost" className="size-7 rounded-md" />}
    />
  );
}
