import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { PressButton } from "@/shared";
import type { Outcome } from "../hooks/call";

/** A drop the backend refused with `role_exists`: the same files, to send again with `replace`. */
export interface RoleConflict {
  paths: string[];
  /** The backend's `role_exists` message. */
  message: string;
}

interface ReplaceRolesDialogProps {
  conflict: RoleConflict | null;
  onClose: () => void;
  /** Sends the same paths again with `replace: true`. */
  onReplace: (paths: string[]) => Promise<Outcome<unknown>>;
}

/** "These roles already exist: … Replace them?" for a drop that would overwrite roles. */
export function ReplaceRolesDialog({ conflict, onClose, onReplace }: ReplaceRolesDialogProps) {
  // Keep the last conflict while the dialog animates out.
  const [shown, setShown] = useState(conflict);
  if (conflict !== null && conflict !== shown) setShown(conflict);

  return (
    <Dialog open={conflict !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        {/* Mounts with the dialog: fresh state each time, and press feedback can attach. */}
        {shown && <ReplaceRolesBody key={shown.paths.join("\n")} conflict={shown} onClose={onClose} onReplace={onReplace} />}
      </DialogContent>
    </Dialog>
  );
}

function ReplaceRolesBody({ conflict, onClose, onReplace }: { conflict: RoleConflict } & Omit<ReplaceRolesDialogProps, "conflict">) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const titles = existingTitles(conflict.message);

  const replace = async () => {
    setBusy(true);
    setError(null);
    const outcome = await onReplace(conflict.paths);
    setBusy(false);
    if (outcome.ok) onClose();
    else setError(outcome.error.message);
  };

  return (
    <>
      <DialogHeader>
        <DialogTitle>Replace existing roles?</DialogTitle>
        <DialogDescription className="text-[13px] leading-snug">
          {/* If the message isn't in the expected shape, show it as-is rather than guess. */}
          {titles !== null ? `These roles already exist: ${titles}. Replace them?` : conflict.message}
        </DialogDescription>
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
        <PressButton type="button" autoFocus disabled={busy} onClick={() => void replace()}>
          Replace
        </PressButton>
      </DialogFooter>
    </>
  );
}

const PREFIX = "the Role Handbook already has ";
const SUFFIX = ": replace them";

/**
 * The titles from the backend's `role_exists` message ("the Role Handbook already has A, B:
 * replace them, or …"). The error carries no structured list, so this reads the message; it
 * takes everything up to the last ": replace them", so titles containing ", " or ":" survive.
 * Null when the message doesn't have that shape.
 */
function existingTitles(message: string): string | null {
  const end = message.lastIndexOf(SUFFIX);
  if (!message.startsWith(PREFIX) || end <= PREFIX.length) return null;
  return message.slice(PREFIX.length, end);
}
