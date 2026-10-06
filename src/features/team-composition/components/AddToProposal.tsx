import { BookUser, Plus } from "lucide-react";
import { useState, type KeyboardEvent } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Popover, PopoverContent, PopoverHeader, PopoverTitle, PopoverTrigger } from "@/components/ui/popover";
import type { CustomRole, HandbookRole } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { HandbookRoles } from "../hooks/useHandbookRoles";
import { cleanCustomRole, EMPTY_CUSTOM_ROLE } from "../lib/draft";
import { CustomRoleFields } from "./CustomRoleFields";
import { HandbookRoleList } from "./HandbookRoleList";

/** "Add from handbook" and "Add custom role" under the proposal cards. Client-side only. */
export function AddToProposal({
  handbook,
  onAddRole,
  onAddCustom,
}: {
  handbook: HandbookRoles;
  onAddRole: (role: HandbookRole) => void;
  onAddCustom: (role: CustomRole) => void;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [customOpen, setCustomOpen] = useState(false);

  return (
    <div className="flex flex-wrap gap-2">
      <Popover
        open={pickerOpen}
        onOpenChange={(open) => {
          setPickerOpen(open);
          if (open) handbook.reload();
        }}
      >
        <PopoverTrigger render={<PressButton size="sm" variant="outline" className="rounded-md" />}>
          <BookUser data-icon="inline-start" />
          Add from handbook
        </PopoverTrigger>
        <PopoverContent align="start" className="w-80 gap-3 p-3">
          <PopoverHeader>
            <PopoverTitle className="text-[13px]">Add from the Role Handbook</PopoverTitle>
          </PopoverHeader>
          <HandbookRoleList
            handbook={handbook}
            autoFocus
            onPick={(role) => {
              setPickerOpen(false);
              onAddRole(role);
            }}
          />
        </PopoverContent>
      </Popover>

      <PressButton size="sm" variant="outline" className="rounded-md" onClick={() => setCustomOpen(true)}>
        <Plus data-icon="inline-start" />
        Add custom role
      </PressButton>
      <Dialog open={customOpen} onOpenChange={setCustomOpen}>
        <DialogContent className="sm:max-w-lg">
          <CustomRoleBody
            onClose={() => setCustomOpen(false)}
            onAdd={(role) => {
              setCustomOpen(false);
              onAddCustom(role);
            }}
          />
        </DialogContent>
      </Dialog>
    </div>
  );
}

function CustomRoleBody({ onClose, onAdd }: { onClose: () => void; onAdd: (role: CustomRole) => void }) {
  const [role, setRole] = useState<CustomRole>(EMPTY_CUSTOM_ROLE);
  const cleaned = cleanCustomRole(role);

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && cleaned) {
      event.preventDefault();
      onAdd(cleaned);
    }
  };

  return (
    <div className="contents" onKeyDown={onKeyDown}>
      <DialogHeader>
        <DialogTitle>Add custom role</DialogTitle>
        <DialogDescription className="text-[13px] leading-snug">
          A role that isn't in your Role Handbook. It's added to this proposal only; nothing is created until you confirm.
        </DialogDescription>
      </DialogHeader>
      <CustomRoleFields value={role} onChange={setRole} autoFocus />
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="button" disabled={!cleaned} onClick={() => cleaned && onAdd(cleaned)}>
          Add role
        </PressButton>
      </DialogFooter>
    </div>
  );
}
