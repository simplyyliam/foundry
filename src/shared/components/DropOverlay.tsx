import { cn } from "@/lib/utils";

/**
 * The "Drop to add" veil shown over a non-empty drop target while files are dragged over it
 * (see `useFileDrop`). Place it last inside a `relative` element. It never takes the pointer.
 */
export function DropOverlay({ active, className }: { active: boolean; className?: string }) {
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none absolute -inset-1.5 flex items-start justify-center rounded-2xl bg-background/70 pt-10 opacity-0 ring-2 ring-ring/40 transition-opacity duration-150",
        active && "opacity-100",
        className,
      )}
    >
      <span className="rounded-full bg-popover px-3 py-1.5 text-[12px] font-medium shadow-md ring-1 ring-foreground/5">
        Drop to add
      </span>
    </div>
  );
}
