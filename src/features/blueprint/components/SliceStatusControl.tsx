import { useCallback, useState } from "react";
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
import type { CommitSetupResult, Plan, SliceStatus, SliceView } from "@/lib/bindings";
import { NotVerifiedDialog, PressButton, usePressScale } from "@/shared";
import { useAction } from "../hooks/useAction";
import { useCommitSetup } from "../hooks/useCommitSetup";
import type { PlanActions } from "../hooks/usePlanActions";
import { STATUS_LABEL } from "../lib/order";
import { ActionError } from "./Notices";
import { CommittedSetup, SetupNotCommitted } from "./SetupCommit";

const STATUSES: SliceStatus[] = ["queued", "in_progress", "done"];

interface SliceStatusControlProps {
  plan: Plan;
  slice: SliceView;
  actions: PlanActions;
}

/**
 * queued / in progress / done. Starting a slice whose dependencies aren't done asks for a
 * reason first: that's the override, and the backend records the reason as a decision.
 * Marking done without a passed verification (`not_verified`) asks for a reason the same way.
 * A start refused with `setup_not_committed` offers "Commit setup", which retries the start.
 */
export function SliceStatusControl({ plan, slice, actions }: SliceStatusControlProps) {
  const { busy, error, run } = useAction();
  const [committed, setCommitted] = useState<CommitSetupResult | null>(null);
  const setup = useCommitSetup(actions.commitSetup, setCommitted);
  const [overrideOpen, setOverrideOpen] = useState(false);
  const [notVerifiedOpen, setNotVerifiedOpen] = useState(false);
  const unfinished = slice.dependsOn
    .map((id) => plan.slices.find((candidate) => candidate.id === id))
    .filter((dependency) => dependency !== undefined && dependency.status !== "done")
    .map((dependency) => dependency!.name);

  const choose = async (status: SliceStatus) => {
    if (status === slice.status || busy || setup.committing) return;
    setCommitted(null);
    setup.clear();
    if (slice.status === "queued" && status === "in_progress" && unfinished.length > 0) {
      setOverrideOpen(true);
      return;
    }
    const outcome = await run(() => actions.setStatus(slice.id, status));
    // The plan moved on under us (a dependency was reopened): ask for the reason after all.
    if (!outcome.ok && outcome.error.code === "dependencies_not_done") setOverrideOpen(true);
    if (!outcome.ok && outcome.error.code === "not_verified") setNotVerifiedOpen(true);
  };

  return (
    <div className="flex flex-col gap-2">
      <div role="group" aria-label="Status" className="flex w-fit gap-0.5 rounded-full bg-muted p-0.5">
        {STATUSES.map((status) => (
          <StatusOption
            key={status}
            status={status}
            active={status === slice.status}
            disabled={busy || setup.committing}
            onClick={() => void choose(status)}
          />
        ))}
      </div>
      {error?.code === "setup_not_committed" && !overrideOpen ? (
        <SetupNotCommitted
          refusal={error}
          committing={setup.committing}
          commitError={setup.error}
          onCommit={() => void setup.commitThen(() => run(() => actions.setStatus(slice.id, "in_progress")))}
        />
      ) : (
        !overrideOpen && !notVerifiedOpen && error?.code !== "not_verified" && <ActionError error={error} />
      )}
      {committed && <CommittedSetup result={committed} onDismiss={() => setCommitted(null)} />}
      <OverrideDialog
        open={overrideOpen}
        onOpenChange={setOverrideOpen}
        sliceName={slice.name}
        unfinished={unfinished}
        onSubmit={(reason) => actions.setStatus(slice.id, "in_progress", reason)}
        commitSetup={actions.commitSetup}
        onCommitted={setCommitted}
      />
      <NotVerifiedDialog
        open={notVerifiedOpen}
        onOpenChange={setNotVerifiedOpen}
        onOverride={(reason) => actions.setStatus(slice.id, "done", reason)}
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
  commitSetup: PlanActions["commitSetup"];
  /** The commit outlives the dialog: the sheet shows it after "Start anyway" closes it. */
  onCommitted: (result: CommitSetupResult) => void;
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
  commitSetup,
  onCommitted,
  onClose,
}: Omit<OverrideDialogProps, "open" | "onOpenChange"> & { onClose: () => void }) {
  const [reason, setReason] = useState("");
  const { busy, error, run } = useAction();
  const setup = useCommitSetup(commitSetup, onCommitted);
  const empty = reason.trim() === "";

  const submit = useCallback(async () => {
    if (empty || busy) return;
    const outcome = await run(() => onSubmit(reason.trim()));
    if (outcome.ok) onClose();
  }, [empty, busy, run, onSubmit, reason, onClose]);

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
        {error?.code === "setup_not_committed" ? (
          <SetupNotCommitted
            refusal={error}
            committing={setup.committing}
            commitError={setup.error}
            onCommit={() => void setup.commitThen(submit)}
          />
        ) : (
          <ActionError error={error} />
        )}
      </div>
      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="submit" disabled={empty || busy || setup.committing}>
          Start anyway
        </PressButton>
      </DialogFooter>
    </form>
  );
}
