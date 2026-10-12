import { CircleCheck, GitCommitHorizontal, LoaderCircle, TriangleAlert } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { AppError, ChangedFile, CommitSliceResult, CommitStatus, FileChange } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "../PressButton";
import type { SliceCommitController } from "./types";

/** How long the success line stays before the dialog closes by itself. */
const CLOSE_AFTER_MS = 2400;

const plural = (count: number, one: string) => `${count} ${count === 1 ? one : `${one}s`}`;

interface CommitSliceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sliceName: string;
  controller: SliceCommitController;
}

/**
 * What `commitSlice` will commit: the files, the generated message (editable: first line and
 * body) and the type it picked with its reason. An edited message is sent exactly as written;
 * an untouched one is left to the backend (`null`), which builds the same text.
 */
export function CommitSliceDialog({ open, onOpenChange, sliceName, controller }: CommitSliceDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        {/* Mounts with the dialog: a fresh read and a fresh draft each time it opens. */}
        <CommitBody sliceName={sliceName} controller={controller} onClose={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

function CommitBody({ sliceName, controller, onClose }: { sliceName: string; controller: SliceCommitController; onClose: () => void }) {
  const { refresh, status } = controller;
  const [fresh, setFresh] = useState(false);
  const [result, setResult] = useState<CommitSliceResult | null>(null);

  // What's shown must be what would be committed now, not the last read.
  useEffect(() => {
    let live = true;
    void refresh().then(() => live && setFresh(true));
    return () => {
      live = false;
    };
  }, [refresh]);

  return (
    <>
      <DialogHeader>
        <DialogTitle>Commit “{sliceName}”</DialogTitle>
        <DialogDescription>Everything uncommitted in the repo is committed. Nothing is pushed.</DialogDescription>
      </DialogHeader>
      {result ? (
        <Committed result={result} onClose={onClose} />
      ) : !fresh || status.status === "loading" ? (
        <div className="flex justify-center py-8">
          <LoaderCircle aria-label="Reading the changes" className="size-4 animate-spin text-muted-foreground" />
        </div>
      ) : status.status === "error" ? (
        <>
          <ErrorLine error={status.error} />
          <DialogFooter>
            <PressButton type="button" variant="ghost" onClick={onClose}>
              Close
            </PressButton>
          </DialogFooter>
        </>
      ) : status.value.total === 0 || status.value.message === null ? (
        <>
          <p className="text-[13px] text-muted-foreground">Nothing to commit.</p>
          <DialogFooter>
            <PressButton type="button" variant="ghost" onClick={onClose}>
              Close
            </PressButton>
          </DialogFooter>
        </>
      ) : (
        <CommitForm status={status.value} message={status.value.message} commit={controller.commit} onCommitted={setResult} onClose={onClose} />
      )}
    </>
  );
}

/** Splits a message into its first line and its body (the blank line between is dropped). */
function splitMessage(message: string) {
  const end = message.indexOf("\n");
  if (end < 0) return { subject: message, body: "" };
  return { subject: message.slice(0, end), body: message.slice(end + 1).replace(/^\n/, "") };
}

const joinMessage = (subject: string, body: string) => (body === "" ? subject : `${subject}\n\n${body}`);

/** The conventional-commit type a first line starts with (`fix(api)!: …` → `fix`), if any. */
function typeOf(subject: string): string | null {
  return /^([A-Za-z]+)(\([^)]*\))?!?:/.exec(subject)?.[1] ?? null;
}

interface CommitFormProps {
  status: CommitStatus;
  message: string;
  commit: SliceCommitController["commit"];
  onCommitted: (result: CommitSliceResult) => void;
  onClose: () => void;
}

function CommitForm({ status, message, commit, onCommitted, onClose }: CommitFormProps) {
  const [original] = useState(() => splitMessage(message));
  const [subject, setSubject] = useState(original.subject);
  const [body, setBody] = useState(original.body);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const edited = subject !== original.subject || body !== original.body;
  const emptySubject = subject.trim() === "";

  // The type follows the first line: the backend's pick until the user retypes it.
  const typedType = typeOf(subject);
  const typeChanged = typedType !== typeOf(original.subject);

  const submit = async () => {
    if (busy || emptySubject) return;
    setBusy(true);
    setError(null);
    const outcome = await commit(edited ? joinMessage(subject, body) : null);
    setBusy(false);
    if (outcome.ok) onCommitted(outcome.data);
    else setError(outcome.error);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void submit();
    }
  };

  return (
    <form
      className="flex min-h-0 flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <FileList status={status} />

      <div className="flex flex-col gap-2">
        <Label htmlFor="commit-subject" className="text-[13px]">
          Message
        </Label>
        <Input
          id="commit-subject"
          aria-label="First line"
          value={subject}
          onChange={(event) => setSubject(event.target.value)}
          onKeyDown={(event) => {
            onKeyDown(event);
            // Enter goes on to the body, like a commit editor; Ctrl/⌘+Enter commits.
            if (event.key === "Enter" && !event.metaKey && !event.ctrlKey) {
              event.preventDefault();
              bodyRef.current?.focus();
            }
          }}
          className="font-mono text-[12.5px]"
        />
        <Textarea
          ref={bodyRef}
          aria-label="Body"
          value={body}
          onChange={(event) => setBody(event.target.value)}
          onKeyDown={onKeyDown}
          className="max-h-60 min-h-28 font-mono text-[12px] leading-relaxed"
        />
        <p className="flex flex-wrap items-baseline gap-x-1.5 gap-y-0.5 text-[12.5px] text-muted-foreground" aria-label="Commit type">
          <span className="font-medium text-foreground">Type</span>
          {typeChanged ? (
            <>
              <TypeChip>{typedType ?? "none"}</TypeChip>
              <span>{typedType ? "from your first line" : "your first line has no type"}</span>
            </>
          ) : (
            <>
              <TypeChip>{status.commitType ?? typedType ?? "none"}</TypeChip>
              {status.typeReason && <span>{status.typeReason}</span>}
            </>
          )}
        </p>
        <p className="text-[12px] text-muted-foreground">Change the type by editing the start of the first line.</p>
      </div>

      {error && <ErrorLine error={error} />}

      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="submit" disabled={busy || emptySubject} title={emptySubject ? "Write a first line" : undefined}>
          {busy ? (
            <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" />
          ) : (
            <GitCommitHorizontal aria-hidden data-icon="inline-start" />
          )}
          {busy ? "Committing…" : "Commit"}
        </PressButton>
      </DialogFooter>
    </form>
  );
}

function TypeChip({ children }: { children: string }) {
  return <code className="rounded-md bg-muted px-1.5 py-px font-mono text-[12px] text-foreground">{children}</code>;
}

const CHANGE_LABEL: Record<FileChange, string> = {
  new: "new",
  modified: "modified",
  deleted: "deleted",
  renamed: "renamed",
};

/** The files to commit: the work first, Foundry's own files in a muted group under it. */
function FileList({ status }: { status: CommitStatus }) {
  const work = status.files.filter((file) => !file.foundryOwned);
  const foundry = status.files.filter((file) => file.foundryOwned);
  const unlisted = status.total - status.files.length;
  return (
    <div className="flex flex-col gap-1.5">
      <p className="text-[12.5px] text-muted-foreground">
        {plural(status.total, "file")} to commit{status.branch && <> on <code className="font-mono text-[12px]">{status.branch}</code></>}
      </p>
      <div className="flex max-h-56 flex-col gap-2 overflow-y-auto rounded-lg bg-muted/50 px-3 py-2">
        {work.length > 0 && <FileGroup label="Changes" files={work} />}
        {foundry.length > 0 && <FileGroup label="Foundry files" files={foundry} muted />}
        {status.truncated && unlisted > 0 && <p className="text-[12px] text-muted-foreground">and {plural(unlisted, "more file")}, not listed</p>}
      </div>
    </div>
  );
}

function FileGroup({ label, files, muted }: { label: string; files: ChangedFile[]; muted?: boolean }) {
  return (
    <section className="flex flex-col gap-0.5">
      <h4 className="text-[11.5px] font-medium text-muted-foreground">{label}</h4>
      <ul aria-label={label} className={cn("flex flex-col gap-0.5 font-mono text-[12px]", muted && "text-muted-foreground")}>
        {files.map((file) => (
          <li key={file.path} className="flex min-w-0 items-baseline gap-2">
            <span className={cn("w-16 shrink-0 text-[11px]", file.change === "deleted" ? "text-destructive" : "text-muted-foreground")}>
              {CHANGE_LABEL[file.change]}
            </span>
            <span className="truncate" title={file.path}>
              {file.path}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** The short id and how many files, then the dialog closes by itself. */
function Committed({ result, onClose }: { result: CommitSliceResult; onClose: () => void }) {
  const short = result.commit.slice(0, 7);
  // The timer starts once: re-renders (the status re-read after the commit) don't restart it.
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);
  useEffect(() => {
    const timer = window.setTimeout(() => close.current(), CLOSE_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, []);

  return (
    <>
      <div role="status" className="flex items-center gap-2.5 rounded-xl bg-status-running/10 px-3.5 py-2.5 text-[13px]">
        <CircleCheck aria-hidden className="size-4 shrink-0 text-status-running" />
        {result.files.length === 0 ? (
          <p>Nothing to commit.</p>
        ) : (
          <p>
            Committed {plural(result.files.length, "file")}
            {short && (
              <>
                {" "}
                as <code className="font-mono text-[12px] select-text">{short}</code>
              </>
            )}
          </p>
        )}
      </div>
      <DialogFooter>
        <PressButton type="button" onClick={onClose}>
          Done
        </PressButton>
      </DialogFooter>
    </>
  );
}

/** The backend's message as-is, selectable (git's fix is a command to copy). */
function ErrorLine({ error }: { error: AppError }) {
  return (
    <div role="alert" className="flex items-start gap-2 rounded-lg bg-destructive/8 px-3 py-2 text-[12.5px]">
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 leading-snug break-words whitespace-pre-wrap select-text">{error.message}</p>
    </div>
  );
}
