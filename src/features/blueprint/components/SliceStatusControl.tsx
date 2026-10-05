import { useState } from "react";
import { Button } from "@/components/ui/button";
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
import { cn } from "@/lib/utils";
import type { Plan, SliceStatus, SliceView } from "@/lib/bindings";
import { PressButton, usePressScale } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { STATUS_LABEL } from "../lib/order";
import { ActionError } from "./Notices";

const STATUSES: SliceStatus[] = ["queued", "in_progress", "done"];

interface SliceStatusControlProps {
  plan: Plan;
  slice: SliceView;
  actions: PlanActions;
}

/**
 * queued / in progress / done. Starting a slice whose dependencies aren't done asks for a
 * reason first: that's the override, and the backend records the reason as a decision.
 */
export function SliceStatusControl({ plan, slice, actions }: SliceStatusControlProps) {
  const { busy, error, run } = useAction();
  const [overrideOpen, setOverrideOpen] = useState(false);
  const unfinished = slice.dependsOn
    .map((id) => plan.slices.find((candidate) => candidate.id === id))
    .filter((dependency) => dependency !== undefined && dependency.status !== "done")
    .map((dependency) => dependency!.name);

  const choose = async (status: SliceStatus) => {
    if (status === slice.status || busy) return;
    if (slice.status === "queued" && status === "in_progress" && unfinished.length > 0) {
      setOverrideOpen(true);
      return;
    }
    const outcome = await run(() => actions.setStatus(slice.id, status));
    // The plan moved on under us (a dependency was reopened): ask for the reason after all.
    if (!outcome.ok && outcome.error.code === "dependencies_not_done") setOverrideOpen(true);
  };

  return (
    <div className="flex flex-col gap-2">
      <div role="group" aria-label="Status" className="flex w-fit gap-0.5 rounded-full bg-muted p-0.5">
        {STATUSES.map((status) => (
          <StatusOption key={status} status={status} active={status === slice.status} disabled={busy} onClick={() => void choose(status)} />
        ))}
      </div>
      {!overrideOpen && <ActionError error={error} />}
      <OverrideDialog
        open={overrideOpen}
        onOpenChange={setOverrideOpen}
        sliceName={slice.name}
        unfinished={unfinished}
        onSubmit={(reason) => actions.setStatus(slice.id, "in_progress", reason)}
      />
    </div>
  );
}

function StatusOption({ status, active, disabled, onClick }: { status: SliceStatus; active: boolean; disabled: boolean; onClick: () => void }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.95 });
  return (
    <Button
      ref={ref}
      size="xs"
      variant="ghost"
      aria-pressed={active}
      disabled={disabled && !active}
      onClick={onClick}
      className={cn("rounded-full px-3", active ? "bg-background text-foreground shadow-sm hover:bg-background" : "text-muted-foreground")}
    >
      {STATUS_LABEL[status]}
    </Button>
  );
}

interface OverrideDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sliceName: string;
  unfinished: string[];
  onSubmit: (reason: string) => ReturnType<PlanActions["setStatus"]>;
}

function OverrideDialog({ open, onOpenChange, ...form }: OverrideDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        {/* Mounts with the dialog: fresh state each time, and press feedback can attach. */}
        <OverrideForm {...form} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function OverrideForm({
  sliceName,
  unfinished,
  onSubmit,
  onClose,
}: Omit<OverrideDialogProps, "open" | "onOpenChange"> & { onClose: () => void }) {
  const [reason, setReason] = useState("");
  const { busy, error, run } = useAction();
  const empty = reason.trim() === "";

  const submit = async () => {
    if (empty || busy) return;
    const outcome = await run(() => onSubmit(reason.trim()));
    if (outcome.ok) onClose();
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
        <DialogTitle>Start “{sliceName}” early?</DialogTitle>
        <DialogDescription>
          {unfinished.length > 0
            ? `It depends on ${unfinished.join(", ")}, which ${unfinished.length === 1 ? "isn't" : "aren't"} done yet.`
            : "Some of its dependencies aren't done yet."}{" "}
          Say why it can start anyway; the reason is kept as a decision on the slice.
        </DialogDescription>
      </DialogHeader>
      <div className="flex flex-col gap-2">
        <Label htmlFor="override-reason" className="text-[13px]">
          Reason
        </Label>
        <Textarea
          id="override-reason"
          autoFocus
          value={reason}
          onChange={(event) => setReason(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              void submit();
            }
          }}
          placeholder="e.g. The API shape is agreed, so the UI can start against a stub."
          className="min-h-20 text-[13px]"
        />
        <ActionError error={error} />
      </div>
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="submit" disabled={empty || busy}>
          Start anyway
        </PressButton>
      </DialogFooter>
    </form>
  );
}
