import { LoaderCircle } from "lucide-react";
import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { PressButton } from "@/shared";

export interface ForceRequest {
  sliceId: string;
  sliceName: string;
  agentId: string;
  /** The unfinished dependencies the backend named, or null if its message couldn't be read. */
  names: string[] | null;
  /** The backend's message, shown whole when `names` is null. */
  message: string;
}

interface ForceDialogProps {
  request: ForceRequest | null;
  onClose: () => void;
  /** Generates with `force: true`. Resolves with null to close, or the backend's message. */
  onConfirm: (request: ForceRequest) => Promise<string | null>;
}

/** "Generate anyway?" after `dependencies_not_done`, listing what isn't done. */
export function ForceDialog({ request, onClose, onConfirm }: ForceDialogProps) {
  // Keep the last request while the dialog animates out.
  const [shown, setShown] = useState(request);
  if (request !== null && request !== shown) setShown(request);

  return (
    <Dialog open={request !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        {/* Mounts with the dialog: fresh state each time, and press feedback can attach. */}
        {shown && <ForceBody request={shown} onClose={onClose} onConfirm={onConfirm} />}
      </DialogContent>
    </Dialog>
  );
}

function ForceBody({ request, onClose, onConfirm }: { request: ForceRequest } & Omit<ForceDialogProps, "request">) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const confirm = async () => {
    setBusy(true);
    setError(null);
    const message = await onConfirm(request);
    setBusy(false);
    if (message !== null) setError(message);
  };

  return (
    <>
      <DialogHeader>
        <DialogTitle>Generate anyway?</DialogTitle>
        <DialogDescription className="text-[13px] leading-snug">
          {request.names ? (
            <>
              {request.sliceName} depends on {request.names.length === 1 ? "a slice that isn't" : "slices that aren't"} done
              yet:
            </>
          ) : (
            request.message
          )}
        </DialogDescription>
      </DialogHeader>
      {request.names && (
        <ul className="flex flex-col gap-1 rounded-lg bg-muted/60 px-3 py-2.5 text-[13px] ring-1 ring-foreground/5">
          {request.names.map((name, index) => (
            <li key={index} className="flex items-center gap-2">
              <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-slice" />
              {name}
            </li>
          ))}
        </ul>
      )}
      <p className="text-[12.5px] leading-snug text-muted-foreground">
        The prompt will say so, and a decision is recorded on {request.sliceName}.
      </p>
      {error && (
        <p role="alert" className="rounded-lg bg-destructive/8 px-3 py-2 text-[13px]">
          {error}
        </p>
      )}
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="button" autoFocus disabled={busy} onClick={() => void confirm()}>
          {busy && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
          Generate anyway
        </PressButton>
      </DialogFooter>
    </>
  );
}
