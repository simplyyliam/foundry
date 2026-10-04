import type { ReactNode } from "react";

/**
 * The document's own header row, above the scrolling area: holds the save status. No title:
 * the page header above already names the project. Kept outside the scroll area so the
 * selection menu treats the scroll area's top edge as the limit and flips below in time.
 */
export function DocumentHeader({ note, children }: { note?: string | null; children?: ReactNode }) {
  return (
    <div className="flex h-8 shrink-0 items-center justify-between gap-4 px-6">
      <span className="min-w-0 truncate text-[12px] text-muted-foreground" title={note ?? undefined}>
        {note}
      </span>
      <div className="flex min-w-0 items-center">{children}</div>
    </div>
  );
}
