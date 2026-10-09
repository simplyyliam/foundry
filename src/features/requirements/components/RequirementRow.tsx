import { useState } from "react";
import { Check, ChevronRight, Globe, MoreHorizontal, Pencil, Rows3, Trash2, Undo2 } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Dialog,
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
import type { PlanRequirement, SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { RequirementActions } from "../hooks/useRequirementActions";
import { KindChip, StateChip, UnsourcedBadge } from "./Chips";
import { ActionError } from "./Notices";
import { isProjectWide } from "../lib/order";
import { RequirementDialog } from "./RequirementDialog";
import { SliceMenu } from "./SliceMenu";

interface RequirementRowProps {
  requirement: PlanRequirement;
  slices: SliceView[];
  actions: RequirementActions;
}

export function RequirementRow({ requirement, slices, actions }: RequirementRowProps) {
  const [expanded, setExpanded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const { busy, error, run } = useAction();
  const assigned = requirement.sliceId !== null && slices.some((slice) => slice.id === requirement.sliceId);
  const projectWide = isProjectWide(requirement);
  // The backend confirms a requirement once it has a slice or is project-wide.
  const handled = assigned || projectWide;
  const confirmed = requirement.state === "confirmed";

  return (
    <li className="rounded-2xl transition-colors hover:bg-muted/40 has-data-[panel-open]:bg-muted/40">
      <Collapsible open={expanded} onOpenChange={setExpanded}>
        <div className="flex items-start gap-2 px-2 py-2.5">
          <CollapsibleTrigger
            render={
              <PressButton
                variant="ghost"
                size="icon-xs"
                pressScale={0.9}
                aria-label={expanded ? "Hide details" : "Show details"}
                className="mt-px shrink-0 rounded-full text-muted-foreground"
              >
                <ChevronRight aria-hidden className={expanded ? "rotate-90 transition-transform" : "transition-transform"} />
              </PressButton>
            }
          />
          <div className="flex min-w-0 flex-1 flex-col gap-1.5">
            <p className="text-[13px] leading-snug text-pretty">{requirement.text}</p>
            <div className="flex flex-wrap items-center gap-1.5">
              <KindChip kind={requirement.kind} />
              <StateChip state={requirement.state} />
              {requirement.unsourced && <UnsourcedBadge />}
              {!handled && <span className="text-[11px] text-muted-foreground">Needs a slice before it can be confirmed</span>}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <SliceMenu
              slices={slices}
              value={assigned ? requirement.sliceId : null}
              projectWide={projectWide}
              disabled={busy}
              unassignResets={confirmed && !projectWide}
              onChange={(sliceId) =>
                // Assigning to a slice turns project-wide off by itself; unassigning doesn't.
                void run(() =>
                  projectWide && sliceId === null
                    ? actions.setProjectWide(requirement.id, false)
                    : actions.assign(requirement.id, sliceId),
                )
              }
            />
            {confirmed ? (
              <PressButton
                variant="ghost"
                size="xs"
                disabled={busy}
                onClick={() => void run(() => actions.unconfirm(requirement.id))}
                className="w-24 text-muted-foreground"
              >
                <Undo2 aria-hidden data-icon="inline-start" />
                Unconfirm
              </PressButton>
            ) : (
              <PressButton
                variant="secondary"
                size="xs"
                disabled={busy || !handled}
                onClick={() => void run(() => actions.confirm(requirement.id))}
                className="w-24"
              >
                <Check aria-hidden data-icon="inline-start" />
                Confirm
              </PressButton>
            )}
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <PressButton variant="ghost" size="icon-xs" pressScale={0.9} aria-label="More actions" className="rounded-full text-muted-foreground">
                    <MoreHorizontal aria-hidden />
                  </PressButton>
                }
              />
              <DropdownMenuContent align="end" className="w-48 min-w-48">
                <DropdownMenuItem onClick={() => setEditing(true)}>
                  <Pencil aria-hidden />
                  Edit…
                </DropdownMenuItem>
                {projectWide ? (
                  <DropdownMenuItem disabled={busy} onClick={() => void run(() => actions.setProjectWide(requirement.id, false))}>
                    <Rows3 aria-hidden />
                    Assign to a slice
                  </DropdownMenuItem>
                ) : (
                  requirement.kind === "constraint" && (
                    <DropdownMenuItem disabled={busy} onClick={() => void run(() => actions.setProjectWide(requirement.id, true))}>
                      <Globe aria-hidden />
                      Make project-wide
                    </DropdownMenuItem>
                  )
                )}
                <DropdownMenuItem variant="destructive" onClick={() => setDeleting(true)}>
                  <Trash2 aria-hidden />
                  Delete…
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
        <CollapsibleContent className="transition-opacity duration-150 data-ending-style:opacity-0 data-starting-style:opacity-0">
          <dl className="flex flex-col gap-3 pr-4 pb-3 pl-10 text-[13px]">
            <div className="flex flex-col gap-1">
              <dt className="text-[12px] text-muted-foreground">From the brain dump</dt>
              <dd className="border-l-2 border-border pl-3 leading-snug text-foreground/85 italic">
                {requirement.sourceExcerpt || <span className="text-muted-foreground not-italic">No excerpt</span>}
              </dd>
            </div>
            <div className="flex flex-col gap-1">
              <dt className="text-[12px] text-muted-foreground">Verified by</dt>
              <dd className="leading-snug text-foreground/85">
                {requirement.verifiedBy || <span className="text-muted-foreground">Not said yet</span>}
              </dd>
            </div>
          </dl>
        </CollapsibleContent>
      </Collapsible>
      <ActionError error={error} className="mx-2 mb-2" />
      <RequirementDialog open={editing} onOpenChange={setEditing} actions={actions} slices={slices} requirement={requirement} />
      <DeleteRequirementDialog open={deleting} onOpenChange={setDeleting} requirement={requirement} actions={actions} />
    </li>
  );
}

interface DeleteRequirementDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  requirement: PlanRequirement;
  actions: RequirementActions;
}

function DeleteRequirementDialog({ open, onOpenChange, ...rest }: DeleteRequirementDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DeleteRequirementForm {...rest} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function DeleteRequirementForm({
  requirement,
  actions,
  onClose,
}: Omit<DeleteRequirementDialogProps, "open" | "onOpenChange"> & { onClose: () => void }) {
  const { busy, error, run } = useAction();
  return (
    <>
      <DialogHeader>
        <DialogTitle>Delete this requirement?</DialogTitle>
        <DialogDescription className="line-clamp-4">“{requirement.text}”</DialogDescription>
      </DialogHeader>
      <ActionError error={error} />
      <DialogFooter>
        <PressButton variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton
          variant="destructive"
          disabled={busy}
          onClick={async () => {
            // On success the row unmounts with the requirement.
            if ((await run(() => actions.remove(requirement.id))).ok) onClose();
          }}
        >
          Delete
        </PressButton>
      </DialogFooter>
    </>
  );
}
