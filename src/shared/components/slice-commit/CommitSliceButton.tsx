import { GitCommitHorizontal, X } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { PressButton } from "../PressButton";
import type { CommitAvailability } from "./types";

interface CommitSliceButtonProps {
  availability: CommitAvailability;
  onOpen: () => void;
  size?: "xs" | "sm";
  variant?: "secondary" | "default";
  label?: string;
}

/** "Commit changes": opens the commit dialog, or says why it can't (as a tooltip). */
export function CommitSliceButton({ availability, onOpen, size = "sm", variant = "secondary", label = "Commit changes" }: CommitSliceButtonProps) {
  const button = (
    <PressButton size={size} variant={variant} disabled={!availability.enabled} onClick={onOpen}>
      <GitCommitHorizontal aria-hidden data-icon="inline-start" />
      {label}
    </PressButton>
  );
  if (availability.enabled) return button;
  return (
    <Tooltip>
      {/* A disabled button gets no pointer events: the wrapper carries the tooltip, and is focusable. */}
      <TooltipTrigger
        render={
          <span
            tabIndex={0}
            aria-label={`${label}: ${availability.reason}`}
            className="inline-flex w-fit rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          />
        }
      >
        {button}
      </TooltipTrigger>
      <TooltipContent>{availability.reason}</TooltipContent>
    </Tooltip>
  );
}

interface CommitSlicePromptProps {
  availability: CommitAvailability;
  /** Opens the dialog (and ends the prompt). It never commits by itself. */
  onOpen: () => void;
  onDismiss: () => void;
  className?: string;
}

/** Shown once after "Mark done" succeeds: "Commit this slice?" */
export function CommitSlicePrompt({ availability, onOpen, onDismiss, className }: CommitSlicePromptProps) {
  return (
    <div role="status" className={cn("flex items-center gap-2 rounded-xl bg-muted/60 py-1.5 pr-1.5 pl-3 text-[13px]", className)}>
      <GitCommitHorizontal aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      <p className="mr-auto min-w-0">Commit this slice?</p>
      <CommitSliceButton availability={availability} onOpen={onOpen} size="xs" variant="default" label="Review commit" />
      <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Not now" onClick={onDismiss} className="rounded-full">
        <X aria-hidden />
      </PressButton>
    </div>
  );
}
