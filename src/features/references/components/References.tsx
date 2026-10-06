import { useCallback, useEffect, useRef, useState } from "react";
import { ImagePlus, TriangleAlert, X } from "lucide-react";
import { cn } from "@/lib/utils";
import type { AppError } from "@/lib/bindings";
import { DropOverlay, PressButton, useFileDrop } from "@/shared";
import { addFromBytes, addFromPaths, useBakery } from "../hooks/useBakery";
import { usePasteImage } from "../hooks/usePasteImage";
import { newestFirst } from "../lib/reference";
import { ReferenceCard } from "./ReferenceCard";
import { ReferenceSheet } from "./ReferenceSheet";

interface ReferencesProps {
  projectId: string;
  /** Panel: two columns, compact cards. Pop-out: more columns, every card editable inline. */
  layout: "panel" | "popout";
  className?: string;
}

/**
 * The project's visual references (the Bakery) as a grid, newest first. Drop image files on
 * it, or paste an image while it has focus. The cards a drop or paste just added open with
 * their label and take fields, the first one focused.
 */
export function References({ projectId, layout, className }: ReferencesProps) {
  const data = useBakery(projectId);
  const rootRef = useRef<HTMLDivElement>(null);
  const [fresh, setFresh] = useState<ReadonlySet<string>>(new Set());
  const [addError, setAddError] = useState<AppError | null>(null);
  const [openId, setOpenId] = useState<string | null>(null);

  const added = useCallback((ids: string[]) => setFresh(new Set(ids)), []);

  const bakery = data.bakery;
  const items = bakery ? newestFirst(bakery.items) : [];
  const firstFresh = items.find((item) => fresh.has(item.id))?.id;

  // Focus the first new card's label once a drop or paste lands. Done here rather than with
  // autoFocus: the card can mount (from the store) before this view knows it's new.
  useEffect(() => {
    if (!firstFresh) return;
    const field = rootRef.current?.querySelector<HTMLElement>(`[data-reference-id="${firstFresh}"] [data-reference-field]`);
    field?.focus();
    field?.scrollIntoView({ block: "nearest" });
  }, [firstFresh]);

  const dragging = useFileDrop(rootRef, async (paths) => {
    setAddError(null);
    const outcome = await addFromPaths(projectId, paths);
    if (outcome.ok) added(outcome.data.added);
    else setAddError(outcome.error);
  });

  const onPaste = usePasteImage(
    useCallback(
      async (images: { name: string; bytes: number[] }[]) => {
        setAddError(null);
        const ids: string[] = [];
        for (const image of images) {
          const outcome = await addFromBytes(projectId, image.name, image.bytes);
          if (!outcome.ok) {
            setAddError(outcome.error);
            break;
          }
          ids.push(...outcome.data.added);
        }
        if (ids.length > 0) added(ids);
      },
      [projectId, added],
    ),
  );

  return (
    <div
      ref={rootRef}
      data-references-root
      // Focusable, so clicking the view then Ctrl+V pastes into it.
      tabIndex={-1}
      onPaste={(event) => void onPaste(event)}
      className={cn("relative flex min-h-full flex-col gap-3 outline-none", className)}
    >
      {data.status === "error" && data.error && <ErrorLine error={data.error} />}
      {addError && <ErrorLine error={addError} onDismiss={() => setAddError(null)} />}

      {data.status === "ready" && bakery && items.length === 0 && <EmptyDropZone active={dragging} />}

      {bakery && items.length > 0 && (
        <ul
          className={cn(
            "grid gap-3",
            layout === "panel" ? "grid-cols-2" : "grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-5",
          )}
        >
          {items.map((item) => (
            <ReferenceCard
              key={item.id}
              projectId={projectId}
              dir={bakery.dir}
              item={item}
              mode={layout === "popout" ? "inline" : fresh.has(item.id) ? "quick" : "compact"}
              onOpen={() => setOpenId(item.id)}
            />
          ))}
        </ul>
      )}

      {/* Over a non-empty grid, a drag shows where the files will go. */}
      {items.length > 0 && <DropOverlay active={dragging} />}

      {bakery && (
        <ReferenceSheet projectId={projectId} dir={bakery.dir} items={bakery.items} itemId={openId} onClose={() => setOpenId(null)} />
      )}
    </div>
  );
}

function EmptyDropZone({ active }: { active: boolean }) {
  return (
    <div
      className={cn(
        "flex min-h-48 flex-1 flex-col items-center justify-center gap-2 rounded-2xl border border-dashed border-muted-foreground/35 px-6 py-8 text-center transition-colors duration-150",
        active && "border-ring bg-muted/60",
      )}
    >
      <ImagePlus aria-hidden className="size-5 text-muted-foreground" />
      <p className="max-w-56 text-[12px] leading-snug text-pretty text-muted-foreground">
        Drop images here, or paste one, to show agents what you have in mind.
      </p>
    </div>
  );
}

function ErrorLine({ error, onDismiss }: { error: AppError; onDismiss?: () => void }) {
  return (
    <div role="alert" className="flex items-start gap-2 rounded-xl bg-destructive/8 py-2 pr-1.5 pl-3 text-[12px]">
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 leading-snug">{error.message}</p>
      {onDismiss && (
        <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={onDismiss} className="-my-0.5 rounded-full">
          <X aria-hidden />
        </PressButton>
      )}
    </div>
  );
}
