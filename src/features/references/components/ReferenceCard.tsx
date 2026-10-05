import { useState } from "react";
import { ImageOff } from "lucide-react";
import { cn } from "@/lib/utils";
import type { BakeryItem } from "@/lib/bindings";
import { usePressScale } from "@/shared";
import { imageSrc, needsNote } from "../lib/reference";
import { ReferenceFields } from "./ReferenceFields";

export type CardMode =
  /** Panel: thumbnail, label and marker only. */
  | "compact"
  /** Panel, right after a drop: label and take inline, the rest behind "More". */
  | "quick"
  /** Pop-out: label, take and skip inline; tags and applies-to behind "More". */
  | "inline";

interface ReferenceCardProps {
  projectId: string;
  dir: string;
  item: BakeryItem;
  mode: CardMode;
  onOpen: () => void;
}

export function ReferenceCard({ projectId, dir, item, mode, onOpen }: ReferenceCardProps) {
  const unlabeled = needsNote(item);
  return (
    // A fresh card in the narrow panel takes the full row, so its fields have room to type in.
    <li data-reference-id={item.id} className={cn("flex min-w-0 flex-col gap-1.5", mode === "quick" && "col-span-2")}>
      <Thumbnail dir={dir} item={item} onOpen={onOpen} />
      {mode === "compact" ? (
        <div className="flex min-w-0 flex-col gap-0.5 px-0.5">
          <p className={cn("truncate text-[12px]", !item.label && "text-muted-foreground")}>{item.label || "No label"}</p>
          {unlabeled && <NeedsNoteMarker />}
        </div>
      ) : (
        <>
          {/* Always takes its line, so fields line up across a row of cards. */}
          <NeedsNoteMarker className={cn("px-0.5", !unlabeled && "invisible")} />
          <ReferenceFields
            projectId={projectId}
            item={item}
            visible={mode === "quick" ? ["label", "take"] : ["label", "take", "skip"]}
            more={mode === "quick" ? ["skip", "tags", "appliesTo"] : ["tags", "appliesTo"]}
          />
        </>
      )}
    </li>
  );
}

export function NeedsNoteMarker({ className }: { className?: string }) {
  return (
    <span className={cn("flex shrink-0 items-center gap-1 text-[10px] text-muted-foreground", className)}>
      <span aria-hidden className="size-1.5 rounded-full bg-slice" />
      needs a note
    </span>
  );
}

function Thumbnail({ dir, item, onOpen }: { dir: string; item: BakeryItem; onOpen: () => void }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.98 });
  const src = imageSrc(dir, item.file);
  // Keyed by src: a renamed file (after a label change) gets a fresh load and error state.
  const [failed, setFailed] = useState<string | null>(null);

  return (
    <button
      ref={ref}
      type="button"
      onClick={onOpen}
      aria-label={`Open ${item.label || item.file}`}
      className="relative aspect-[4/3] w-full overflow-hidden rounded-xl bg-muted outline-none ring-1 ring-foreground/5 focus-visible:ring-3 focus-visible:ring-ring/40"
    >
      {failed === src ? (
        <span className="flex h-full flex-col items-center justify-center gap-1 px-2 text-[11px] text-muted-foreground">
          <ImageOff aria-hidden className="size-4" />
          <span className="truncate">{item.file}</span>
        </span>
      ) : (
        <img
          src={src}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => setFailed(src)}
          className="h-full w-full object-cover"
        />
      )}
    </button>
  );
}
