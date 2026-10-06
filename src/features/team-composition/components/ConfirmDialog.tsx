import { LoaderCircle } from "lucide-react";
import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { PressButton } from "@/shared";

interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel: string;
  destructive?: boolean;
  /**
   * Runs on the confirm button. Resolve with null to close, or with the backend's message to
   * keep the dialog open and show it. Synchronous actions just return.
   */
  onConfirm: () => Promise<string | null> | void;
}

/** Asks before doing something that costs usage or can't be undone. */
export function ConfirmDialog({ open, onOpenChange, ...body }: ConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        {/* Mounts with the dialog: fresh state each time, and press feedback can attach. */}
        <ConfirmBody {...body} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function ConfirmBody({
  title,
  description,
  confirmLabel,
  destructive,
  onConfirm,
  onClose,
}: Omit<ConfirmDialogProps, "open" | "onOpenChange"> & { onClose: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const confirm = async () => {
    setBusy(true);
    setError(null);
    const message = await onConfirm();
    setBusy(false);
    if (message) setError(message);
    else onClose();
  };

  return (
    <>
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription className="text-[13px] leading-snug">{description}</DialogDescription>
      </DialogHeader>
      {error && (
        <p role="alert" className="rounded-lg bg-destructive/8 px-3 py-2 text-[13px]">
          {error}
        </p>
      )}
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton
          type="button"
          autoFocus
          variant={destructive ? "destructive" : "default"}
          disabled={busy}
          onClick={() => void confirm()}
        >
          {busy && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
          {confirmLabel}
        </PressButton>
      </DialogFooter>
    </>
  );
}
