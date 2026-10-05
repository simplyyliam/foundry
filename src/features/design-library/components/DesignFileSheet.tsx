import { Trash2 } from "lucide-react";
import { useId, useState, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Textarea } from "@/components/ui/textarea";
import type { DesignFile, DesignFilePatch, DesignLibrary } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { Outcome } from "../hooks/call";
import { useSaveState } from "../hooks/useSaveState";
import { SaveStatus } from "./SaveStatus";
import { TagEditor } from "./TagEditor";

interface DesignFileSheetProps {
  library: DesignLibrary;
  /** The file to show, or null when closed. */
  fileId: string | null;
  onClose: () => void;
  update: (id: string, change: Partial<DesignFilePatch>) => Promise<Outcome<DesignLibrary>>;
  remove: (id: string) => Promise<Outcome<DesignLibrary>>;
}

/** One library file: its filename, description and family tags, and Remove. */
export function DesignFileSheet({ library, fileId, onClose, update, remove }: DesignFileSheetProps) {
  // Keep showing the last file while the sheet animates out.
  const [shown, setShown] = useState<string | null>(fileId);
  if (fileId !== null && fileId !== shown) setShown(fileId);
  const file = library.files.find((candidate) => candidate.id === shown) ?? null;

  return (
    <Sheet open={fileId !== null && file !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full gap-0 sm:max-w-md">
        {file && (
          <Details
            key={file.id}
            file={file}
            suggestions={library.families}
            update={update}
            remove={remove}
            onRemoved={onClose}
          />
        )}
      </SheetContent>
    </Sheet>
  );
}

interface DetailsProps {
  file: DesignFile;
  suggestions: readonly string[];
  update: DesignFileSheetProps["update"];
  remove: DesignFileSheetProps["remove"];
  onRemoved: () => void;
}

function Details({ file, suggestions, update, remove, onRemoved }: DetailsProps) {
  const id = useId();
  const { state, track } = useSaveState();

  return (
    <>
      <SheetHeader className="gap-1 pr-14">
        <SheetTitle className="truncate text-base">{file.filename}</SheetTitle>
        <SheetDescription className="flex min-h-4.5 items-center gap-2 text-[12px]">
          <SaveStatus state={state} />
        </SheetDescription>
      </SheetHeader>

      <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-6 pb-6">
        <p className="rounded-lg bg-muted px-3 py-2 text-[12px] leading-snug text-muted-foreground">
          Changes save on their own: tags as you add or remove them, the description when you leave the field.
        </p>

        <div className="flex flex-col gap-1.5">
          <label htmlFor={`${id}-filename`} className="text-[12px] font-medium text-muted-foreground">
            Filename
          </label>
          <Input id={`${id}-filename`} value={file.filename} readOnly className="h-9 rounded-lg font-mono text-[12.5px] text-muted-foreground md:text-[12.5px]" />
        </div>

        <DescriptionField
          id={`${id}-description`}
          value={file.description}
          onCommit={(description) => track(update(file.id, { description }))}
        />

        <TagsField
          id={`${id}-tags`}
          tags={file.families}
          suggestions={suggestions}
          onCommit={(families) => track(update(file.id, { families }))}
        />

        <RemoveFile file={file} remove={remove} onRemoved={onRemoved} />
      </div>
    </>
  );
}

function DescriptionField({
  id,
  value,
  onCommit,
}: {
  id: string;
  value: string;
  onCommit: (text: string) => Promise<Outcome<unknown>>;
}) {
  // null while not editing, so the field follows the saved text (and goes back to it on an error).
  const [draft, setDraft] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const commit = async () => {
    if (draft === null) return;
    const text = draft.trim();
    setDraft(null);
    if (text === value) return;
    setPending(text);
    setError(null);
    const outcome = await onCommit(text);
    setPending(null);
    if (!outcome.ok) setError(outcome.error.message);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape" && draft !== null && draft !== value) {
      // Put the saved text back instead of closing the sheet.
      event.stopPropagation();
      setDraft(null);
    } else if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void commit();
    }
  };

  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-[12px] font-medium text-muted-foreground">
        Description
      </label>
      <Textarea
        id={id}
        value={draft ?? pending ?? value}
        placeholder="What this guideline covers"
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => void commit()}
        onKeyDown={onKeyDown}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? `${id}-error` : undefined}
        className="min-h-20 text-[13px] md:text-[13px]"
      />
      {error && (
        <p id={`${id}-error`} role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

function TagsField({
  id,
  tags,
  suggestions,
  onCommit,
}: {
  id: string;
  tags: string[];
  suggestions: readonly string[];
  onCommit: (tags: string[]) => Promise<Outcome<unknown>>;
}) {
  const [pending, setPending] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const change = async (next: string[]) => {
    setPending(next);
    setError(null);
    const outcome = await onCommit(next);
    setPending(null);
    if (!outcome.ok) setError(outcome.error.message);
  };

  return (
    <div className="flex flex-col gap-1.5">
      <span id={`${id}-label`} className="text-[12px] font-medium text-muted-foreground">
        Families
      </span>
      <TagEditor
        tags={pending ?? tags}
        suggestions={suggestions}
        onChange={(next) => void change(next)}
        labelId={`${id}-label`}
        describedBy={`${id}-hint${error ? ` ${id}-error` : ""}`}
      />
      <p id={`${id}-hint`} className="text-[12px] text-muted-foreground">
        A project gets this file when its design family matches one of these. With no tags, it's never copied.
      </p>
      {error && (
        <p id={`${id}-error`} role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

function RemoveFile({
  file,
  remove,
  onRemoved,
}: {
  file: DesignFile;
  remove: DesignFileSheetProps["remove"];
  onRemoved: () => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const confirm = async () => {
    setBusy(true);
    const outcome = await remove(file.id);
    setBusy(false);
    if (outcome.ok) onRemoved();
    else {
      setError(outcome.error.message);
      setConfirming(false);
    }
  };

  return (
    <section className="mt-1 flex flex-col gap-2 border-t border-border/60 pt-4">
      {confirming ? (
        <div className="flex flex-wrap items-center gap-2">
          <p className="mr-auto text-[13px]">Remove {file.filename} from the library?</p>
          <PressButton variant="ghost" size="sm" onClick={() => setConfirming(false)}>
            Cancel
          </PressButton>
          <PressButton variant="destructive" size="sm" disabled={busy} onClick={() => void confirm()}>
            Remove
          </PressButton>
        </div>
      ) : (
        <PressButton
          variant="ghost"
          size="sm"
          className="w-fit text-destructive hover:text-destructive"
          onClick={() => {
            setError(null);
            setConfirming(true);
          }}
        >
          <Trash2 aria-hidden data-icon="inline-start" />
          Remove from library
        </PressButton>
      )}
      {error && (
        <p role="alert" className="rounded-lg bg-destructive/8 px-3 py-2 text-[13px]">
          {error}
        </p>
      )}
    </section>
  );
}
