import { ChevronDown, OctagonX, Square } from "lucide-react";
import { useState } from "react";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";

export type Stopping = "graceful" | "force";

interface StopControlsProps {
  agentName: string;
  /** Stop is only offered while the session is running. */
  running: boolean;
  stopping: Stopping | undefined;
  onStop: (force: boolean) => void;
  className?: string;
}

/** Stop (Ctrl+C, then a kill after 5 s), with "Force stop" (kill at once) in a menu, confirmed. */
export function StopControls({ agentName, running, stopping, onStop, className }: StopControlsProps) {
  const [confirming, setConfirming] = useState(false);

  return (
    <div className={cn("flex items-center", className)}>
      <PressButton
        variant="ghost"
        size="icon-sm"
        aria-label={stopping === "graceful" ? `Stopping ${agentName}` : `Stop ${agentName}`}
        title={stopping === "graceful" ? "Stopping…" : "Stop"}
        disabled={!running || stopping !== undefined}
        onClick={() => onStop(false)}
        className="size-7 rounded-r-none hover:bg-foreground/8"
      >
        {stopping === "graceful" ? (
          <span aria-hidden className="size-3 animate-spin rounded-full border-2 border-current border-t-transparent" />
        ) : (
          <Square aria-hidden className="size-3 fill-current" />
        )}
      </PressButton>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <PressButton
              variant="ghost"
              size="icon-sm"
              aria-label="More ways to stop"
              disabled={!running || stopping === "force"}
              className="h-7 w-5 rounded-l-none hover:bg-foreground/8"
            />
          }
        >
          <ChevronDown aria-hidden className="size-3" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-40 min-w-40">
          <DropdownMenuItem variant="destructive" onClick={() => setConfirming(true)}>
            <OctagonX aria-hidden />
            Force stop
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <Dialog open={confirming} onOpenChange={setConfirming}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Force stop {agentName}?</DialogTitle>
            <DialogDescription>
              This kills the session and everything it started right away, without letting Claude finish what it's
              doing.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose render={<PressButton variant="outline" />}>Cancel</DialogClose>
            <PressButton
              variant="destructive"
              onClick={() => {
                setConfirming(false);
                onStop(true);
              }}
            >
              Force stop
            </PressButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
