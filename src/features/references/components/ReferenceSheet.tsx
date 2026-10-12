import { useState } from "react";
import { Trash2 } from "lucide-react";
import { Label } from "@/components/ui/label";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { AppError, BakeryItem } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { removeReference } from "../hooks/useBakery";
import { FIELDS, type FieldKey } from "../lib/fields";
import { imageSrc, needsNote } from "../lib/reference";
import { NeedsNoteMarker } from "./ReferenceCard";
import { Field } from "./ReferenceFields";

const ORDER: FieldKey[] = ["label", "take", "skip", "tags", "appliesTo"];

interface ReferenceSheetProps {
  projectId: string;
  dir: string;
  items: BakeryItem[];
  /** The reference to show, or null when closed. */
  itemId: string | null;
  onClose: () => void;
}

/** One reference: the larger image, every field, and Delete. */
export function ReferenceSheet({ projectId, dir, items, itemId, onClose }: ReferenceSheetProps) {
  // Keep showing the last reference while the sheet animates out.
  const [shown, setShown] = useState<string | null>(itemId);
  if (itemId !== null && itemId !== shown) setShown(itemId);
  const item = items.find((candidate) => candidate.id === shown) ?? null;

  return (
    <Sheet open={itemId !== null && item !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full gap-0 sm:max-w-md">
        {item && <Details key={item.id} projectId={projectId} dir={dir} item={item} onRemoved={onClose} />}
      </SheetContent>
    </Sheet>
  );
}

function Details({ projectId, dir, item, onRemoved }: { projectId: string; dir: string; item: BakeryItem; onRemoved: () => void }) {
  return (
    <>
      <SheetHeader className="gap-1 pr-14">
        <SheetTitle className="truncate text-base">{item.label || "No label"}</SheetTitle>
        <SheetDescription className="flex items-center gap-2 text-[12px]">
          <span className="truncate">{item.file}</span>
          {needsNote(item) && <NeedsNoteMarker />}
        </SheetDescription>
      </SheetHeader>
      <div data-references-root className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-6 pb-6">
        <img
          src={imageSrc(dir, item.file)}
          alt={item.label}
          draggable={false}
          className="max-h-[50vh] w-full rounded-xl bg-muted object-contain ring-1 ring-foreground/5"
        />
        <div className="flex flex-col gap-3">
          {ORDER.map((key) => (
            <div key={key} className="flex flex-col gap-1.5">
              <Label className="text-[12px] text-muted-foreground">{FIELDS[key].label}</Label>
              <Field projectId={projectId} item={item} field={key} multiline={key === "take" || key === "skip"} showHint />
            </div>
          ))}
        </div>
        <DeleteReference projectId={projectId} item={item} onRemoved={onRemoved} />
      </div>
    </>
  );
}

function DeleteReference({ projectId, item, onRemoved }: { projectId: string; item: BakeryItem; onRemoved: () => void }) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const remove = async () => {
    setBusy(true);
    const outcome = await removeReference(projectId, item.id);
    setBusy(false);
    if (outcome.ok) onRemoved();
    else {
      setError(outcome.error);
      setConfirming(false);
    }
  };

  return (
    <section className="mt-1 flex flex-col gap-2 border-t border-border/60 pt-4">
      {confirming ? (
        <div className="flex flex-wrap items-center gap-2">
          <p className="mr-auto text-[13px]">Delete this reference and its image file?</p>
          <PressButton variant="ghost" size="sm" onClick={() => setConfirming(false)}>
            Cancel
          </PressButton>
          <PressButton variant="destructive" size="sm" disabled={busy} onClick={() => void remove()}>
            Delete
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
          Delete
        </PressButton>
      )}
      {error && (
        <p role="alert" className="rounded-xl bg-destructive/8 px-3 py-2 text-[13px]">
          {error.message}
        </p>
      )}
    </section>
  );
}
