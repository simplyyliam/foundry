import { TriangleAlert } from "lucide-react";
import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { AppError } from "@/lib/bindings";
import { PressButton } from "./PressButton";

export type OverrideOutcome = { ok: true } | { ok: false; error: AppError };

interface NotVerifiedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /**
   * Repeats the status change with `override: true` and this reason. The caller makes the
   * call (only feature hooks call bindings); a refusal stays in the dialog, as-is.
   */
  onOverride: (reason: string) => Promise<OverrideOutcome>;
}

/**
 * Asked after `setSliceStatus(done)` answers `not_verified`: mark it done anyway, with a
 * required reason the backend records as a decision. Used by the Verification page and the
 * Blueprint's status control.
 */
export function NotVerifiedDialog({ open, onOpenChange, onOverride }: NotVerifiedDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        {/* Mounts with the dialog: fresh state each time, and press feedback can attach. */}
        <OverrideForm onOverride={onOverride} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function OverrideForm({ onOverride, onClose }: { onOverride: NotVerifiedDialogProps["onOverride"]; onClose: () => void }) {
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const empty = reason.trim() === "";

  const submit = async () => {
    if (empty || busy) return;
    setBusy(true);
    setError(null);
    const outcome = await onOverride(reason.trim());
    setBusy(false);
    if (outcome.ok) onClose();
    else setError(outcome.error);
  };

  return (
    <form
      className="contents"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <DialogHeader>
        <DialogTitle>Mark it done anyway?</DialogTitle>
        <DialogDescription>
          This slice has no passed verification. Mark it done anyway? The reason is kept as a decision on the slice.
        </DialogDescription>
      </DialogHeader>
      <div className="flex flex-col gap-2">
        <Label htmlFor="not-verified-reason" className="text-[13px]">
          Reason
        </Label>
        <Textarea
          id="not-verified-reason"
          autoFocus
          required
          value={reason}
          onChange={(event) => setReason(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              void submit();
            }
          }}
          placeholder="e.g. Checked by hand: the test runner can't reach the device lab from here."
          className="min-h-20 text-[13px]"
        />
        {error && (
          <div role="alert" className="flex items-start gap-2 rounded-xl bg-destructive/8 px-3 py-2 text-[13px]">
            <TriangleAlert aria-hidden className="mt-0.5 size-4 shrink-0 text-destructive" />
            <p className="min-w-0 flex-1 py-px">{error.message}</p>
          </div>
        )}
      </div>
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="submit" disabled={empty || busy}>
          Mark done anyway
        </PressButton>
      </DialogFooter>
    </form>
  );
}
