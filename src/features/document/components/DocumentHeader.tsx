import type { ReactNode } from "react";

/**
 * The document's own header row: sticks to the top while scrolling and holds the save
 * status. No title: the page header above already names the project.
 */
export function DocumentHeader({ note, children }: { note?: string | null; children?: ReactNode }) {
  return (
    <div className="sticky top-0 z-10 flex h-8 shrink-0 items-center justify-between gap-4 bg-background/85 px-6 backdrop-blur-sm">
      <span className="min-w-0 truncate text-[12px] text-muted-foreground" title={note ?? undefined}>
        {note}
      </span>
      <div className="flex min-w-0 items-center">{children}</div>
    </div>
  );
}
