import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { PressButton, requestPanel } from "@/shared";

interface UnsettledConflictsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** `Plan.unresolvedConflicts`. */
  count: number;
  projectId: string;
  onConfirmAnyway: () => void;
}

/**
 * Asked before Confirm all while roadmap conflicts have no ruling. Never blocks: "Settle them"
 * opens the Roadmap's conflicts (in the panel's pop-out), "Confirm anyway" goes ahead.
 */
export function UnsettledConflictsDialog({ open, onOpenChange, count, projectId, onConfirmAnyway }: UnsettledConflictsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{count === 1 ? "1 conflict is unsettled." : `${count} conflicts are unsettled.`}</DialogTitle>
          <DialogDescription>Agents will pick on their own.</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <PressButton
            variant="ghost"
            onClick={() => {
              onOpenChange(false);
              onConfirmAnyway();
            }}
          >
            Confirm anyway
          </PressButton>
          <PressButton
            autoFocus
            onClick={() => {
              onOpenChange(false);
              requestPanel({ kind: "settle-conflicts", projectId });
            }}
          >
            Settle them
          </PressButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
