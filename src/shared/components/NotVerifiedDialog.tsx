import { ChevronDown, FileText, TriangleAlert } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
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
  /** Reasons to start from (the guidance's `reasonTemplate`s); `<reason>` is for the user to fill in. */
  templates?: readonly string[];
  /** What the override sets aside, in plain words: "You are overriding: …". */
  overriding?: readonly string[];
}

/** The part of a template the user still has to write. */
const BLANK = "<reason>";

/**
 * Asked after `setSliceStatus(done)` answers `not_verified`: mark it done anyway, with a
 * required reason the backend records as a decision. Used by the Verification page and the
 * Blueprint's status control.
 */
export function NotVerifiedDialog({ open, onOpenChange, onOverride, templates = [], overriding = [] }: NotVerifiedDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        {/* Mounts with the dialog: fresh state each time, and press feedback can attach. */}
        <OverrideForm onOverride={onOverride} templates={templates} overriding={overriding} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function OverrideForm({
  onOverride,
  templates,
  overriding,
  onClose,
}: Required<Pick<NotVerifiedDialogProps, "onOverride" | "templates" | "overriding">> & { onClose: () => void }) {
  const [reason, setReason] = useState("");
  const reasonRef = useRef<HTMLTextAreaElement>(null);
  // After a template fills the box, select its `<reason>` so typing replaces it.
  const selectBlank = useRef(false);
  const choices = [...new Set(templates.map((template) => template.trim()).filter((template) => template !== ""))];

  useEffect(() => {
    if (!selectBlank.current) return;
    selectBlank.current = false;
    const box = reasonRef.current;
    const at = reason.indexOf(BLANK);
    if (!box) return;
    box.focus();
    if (at >= 0) box.setSelectionRange(at, at + BLANK.length);
    else box.setSelectionRange(reason.length, reason.length);
  }, [reason]);
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
      {overriding.length > 0 && (
        <div className="rounded-lg bg-muted/50 px-3 py-2 text-[12.5px] leading-snug">
          <p className="font-medium">You are overriding:</p>
          <ul className="mt-1 flex list-disc flex-col gap-0.5 pl-4 text-muted-foreground">
            {overriding.map((line, index) => (
              <li key={index} className="text-pretty">
                {line}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-2">
          <Label htmlFor="not-verified-reason" className="text-[13px]">
            Reason
          </Label>
          {choices.length > 0 && (
            <DropdownMenu>
              <DropdownMenuTrigger render={<PressButton type="button" size="xs" variant="ghost" className="-my-1 text-muted-foreground" />}>
                <FileText aria-hidden data-icon="inline-start" />
                Use a template
                <ChevronDown aria-hidden data-icon="inline-end" />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-80" finalFocus={reasonRef}>
                {choices.map((template) => (
                  <DropdownMenuItem
                    key={template}
                    className="items-start text-[12.5px] leading-snug font-normal"
                    onClick={() => {
                      selectBlank.current = true;
                      setReason(template);
                    }}
                  >
                    <span className="line-clamp-3 text-pretty">{template}</span>
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>
        <Textarea
          ref={reasonRef}
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
