import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { ActionError } from "./Notices";

interface AddSliceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: PlanActions;
  /** Called with the new slice's id, so its sheet can open. */
  onAdded: (sliceId: string) => void;
}

export function AddSliceDialog({ open, onOpenChange, actions, onAdded }: AddSliceDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <AddSliceForm actions={actions} onAdded={onAdded} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function AddSliceForm({ actions, onAdded, onClose }: Omit<AddSliceDialogProps, "open" | "onOpenChange"> & { onClose: () => void }) {
  const [name, setName] = useState("");
  const { busy, error, run } = useAction();

  return (
    <form
      className="contents"
      onSubmit={async (event) => {
        event.preventDefault();
        if (busy || !name.trim()) return;
        const outcome = await run(() => actions.addSlice(name.trim()));
        if (outcome.ok) {
          onClose();
          onAdded(outcome.data.id);
        }
      }}
    >
      <DialogHeader>
        <DialogTitle>Add slice</DialogTitle>
        <DialogDescription>A thin piece of the product you can build and check on its own.</DialogDescription>
      </DialogHeader>
      <div className="flex flex-col gap-2">
        <Input
          autoFocus
          value={name}
          aria-label="Slice name"
          placeholder="Slice name"
          onChange={(event) => setName(event.target.value)}
        />
        <ActionError error={error} />
      </div>
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="submit" disabled={busy || !name.trim()}>
          Add slice
        </PressButton>
      </DialogFooter>
    </form>
  );
}
