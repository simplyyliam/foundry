import { FilePlus2, LoaderCircle, TriangleAlert, X } from "lucide-react";
import { useRef, useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import type { DesignFile } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton, useFileDrop } from "@/shared";
import { useDesignLibrary } from "../hooks/useDesignLibrary";
import { DesignFileSheet } from "./DesignFileSheet";

/**
 * The Design Library: your own guideline `.md` files, shared by every project. Drop files
 * anywhere on it to add them; click one to describe and tag it.
 */
export function DesignLibraryView() {
  const { library, loadError, reload, add, update, remove } = useDesignLibrary();
  const rootRef = useRef<HTMLDivElement>(null);
  const [addError, setAddError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [openId, setOpenId] = useState<string | null>(null);

  const dragging = useFileDrop(rootRef, async (paths) => {
    setAddError(null);
    setAdding(true);
    const outcome = await add(paths);
    setAdding(false);
    // All or nothing: the backend's own message when it refuses (e.g. a file that isn't .md).
    if (!outcome.ok) setAddError(outcome.error.message);
  });

  const files = library?.files ?? [];
  const untagged = library?.untaggedCount ?? 0;

  return (
    <div ref={rootRef} className="relative flex min-h-[26rem] flex-col gap-3 pt-3">
      {loadError && (
        <ErrorLine message={loadError}>
          <PressButton size="xs" variant="outline" className="rounded-md" onClick={() => void reload()}>
            Try again
          </PressButton>
        </ErrorLine>
      )}
      {addError && (
        <ErrorLine message={addError}>
          <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={() => setAddError(null)} className="rounded-md">
            <X aria-hidden />
          </PressButton>
        </ErrorLine>
      )}

      {untagged > 0 && (
        <div role="status" className="flex items-start gap-2 rounded-lg bg-muted px-3 py-2.5 text-[12.5px]">
          <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-muted-foreground" />
          <p>
            {untagged === 1
              ? "1 file has no tags and won't be copied into any project."
              : `${untagged} files have no tags and won't be copied into any project.`}
          </p>
        </div>
      )}

      {!library && !loadError ? (
        <div className="flex justify-center py-10">
          <LoaderCircle aria-label="Loading the design library" className="size-4 animate-spin text-muted-foreground" />
        </div>
      ) : library && files.length === 0 ? (
        <DropZone active={dragging} busy={adding} className="min-h-56 flex-1">
          Add your own design guideline files. Tag each one with a family so projects can use it.
        </DropZone>
      ) : (
        files.length > 0 && (
          <>
            <ul className="flex flex-col" aria-label="Design guideline files">
              {files.map((file) => (
                <FileRow key={file.id} file={file} onOpen={() => setOpenId(file.id)} />
              ))}
            </ul>
            <DropZone active={dragging} busy={adding} className="py-4">
              Drop .md files here to add more.
            </DropZone>
          </>
        )
      )}

      {library && (
        <DesignFileSheet library={library} fileId={openId} onClose={() => setOpenId(null)} update={update} remove={remove} />
      )}
    </div>
  );
}

function FileRow({ file, onOpen }: { file: DesignFile; onOpen: () => void }) {
  return (
    <li className="border-b border-border/60 last:border-b-0">
      <PressButton
        variant="ghost"
        pressScale={0.99}
        onClick={onOpen}
        className="my-1 h-auto w-full flex-col items-stretch gap-1 rounded-md px-2.5 py-2.5 text-left font-normal whitespace-normal active:not-aria-[haspopup]:translate-y-0"
      >
        <span className="truncate font-mono text-[13px] font-medium">{file.filename}</span>
        {file.description && <span className="line-clamp-2 text-[12.5px] text-muted-foreground">{file.description}</span>}
        <span className="flex flex-wrap gap-1 pt-0.5">
          {file.families.length === 0 ? (
            <span className="text-[12px] text-muted-foreground italic">No tags</span>
          ) : (
            file.families.map((tag) => (
              <Badge key={tag} variant="secondary" className="h-5 rounded-md px-1.5 text-[11.5px]">
                {tag}
              </Badge>
            ))
          )}
        </span>
      </PressButton>
    </li>
  );
}

function DropZone({ active, busy, className, children }: { active: boolean; busy: boolean; className?: string; children: string }) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-muted-foreground/35 px-6 text-center transition-colors duration-150",
        active && "border-ring bg-muted/60",
        className,
      )}
    >
      {busy ? (
        <LoaderCircle aria-label="Adding files" className="size-5 animate-spin text-muted-foreground" />
      ) : (
        <FilePlus2 aria-hidden className="size-5 text-muted-foreground" />
      )}
      <p className="max-w-72 text-[12.5px] leading-snug text-pretty text-muted-foreground">{active ? "Drop to add" : children}</p>
    </div>
  );
}

function ErrorLine({ message, children }: { message: string; children?: ReactNode }) {
  return (
    <div role="alert" className="flex items-start gap-2 rounded-lg bg-destructive/8 py-2 pr-1.5 pl-3 text-[12.5px]">
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 leading-snug">{message}</p>
      {children}
    </div>
  );
}
