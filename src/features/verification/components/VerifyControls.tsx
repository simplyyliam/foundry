import { ListChecks, ShieldCheck } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { AppError } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { clearVerifyError, verify } from "../hooks/verifyRuns";
import { DismissButton, ErrorLine } from "./Notices";

interface VerifyButtonProps {
  projectId: string;
  sliceId: string;
  reportPresent: boolean;
  /** This slice is being verified right now. */
  running: boolean;
  /** It was verified before: "Verify again". */
  again?: boolean;
  size?: "xs" | "sm";
}

/**
 * Starts a verification. Only missing reports are stopped here (there's nothing to check);
 * every other refusal comes from the backend and shows under the control.
 */
export function VerifyButton({ projectId, sliceId, reportPresent, running, again, size = "xs" }: VerifyButtonProps) {
  const button = (
    <PressButton
      size={size}
      variant="secondary"
      disabled={!reportPresent || running}
      onClick={(event) => {
        event.stopPropagation();
        void verify(projectId, sliceId);
      }}
    >
      <ShieldCheck aria-hidden data-icon="inline-start" />
      {again ? "Verify again" : "Verify"}
    </PressButton>
  );
  if (reportPresent) return button;
  return (
    <Tooltip>
      {/* A disabled button gets no pointer events: the wrapper carries the tooltip, and is focusable. */}
      <TooltipTrigger render={<span tabIndex={0} className="inline-flex rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring/50" />}>
        {button}
      </TooltipTrigger>
      <TooltipContent>No report yet</TooltipContent>
    </Tooltip>
  );
}

interface VerifyErrorProps {
  projectId: string;
  sliceId: string;
  error: AppError | undefined;
  onReviewCommands: () => void;
  className?: string;
}

/** A refused or failed Verify for this slice, as-is. Unconfirmed commands link to the card. */
export function VerifyError({ projectId, sliceId, error, onReviewCommands, className }: VerifyErrorProps) {
  if (!error) return null;
  return (
    <ErrorLine message={error.message} className={className}>
      {error.code === "test_commands_unconfirmed" && (
        <PressButton size="xs" variant="outline" className="shrink-0 rounded-md bg-background" onClick={onReviewCommands}>
          <ListChecks aria-hidden data-icon="inline-start" />
          Review test commands
        </PressButton>
      )}
      <DismissButton onClick={() => clearVerifyError(projectId, sliceId)} />
    </ErrorLine>
  );
}
