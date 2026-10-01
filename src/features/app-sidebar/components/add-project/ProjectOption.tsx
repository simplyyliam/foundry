import { LoaderCircle, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { usePressScale } from "@/shared";

interface ProjectOptionProps {
  id: string;
  icon: LucideIcon;
  label: ReactNode;
  /** Muted text on the right, e.g. "git" or "Added". */
  hint?: string;
  /** Highlighted by the keyboard or the pointer. */
  active: boolean;
  /** This row's request is in flight. */
  busy?: boolean;
  /** Another row's request is in flight. */
  disabled?: boolean;
  onHover: () => void;
  onChoose: () => void;
}

/** One row in the search results. Rows aren't focusable: the search field keeps focus. */
export function ProjectOption({
  id,
  icon: Icon,
  label,
  hint,
  active,
  busy = false,
  disabled = false,
  onHover,
  onChoose,
}: ProjectOptionProps) {
  const ref = usePressScale<HTMLLIElement>({ scale: 0.985, disabled });

  return (
    <li
      ref={ref}
      id={id}
      role="option"
      aria-selected={active}
      aria-disabled={disabled || undefined}
      aria-busy={busy || undefined}
      data-active={active || undefined}
      onPointerMove={onHover}
      // Keep focus in the search field.
      onPointerDown={(event) => event.preventDefault()}
      onClick={disabled ? undefined : onChoose}
      className="flex h-9 cursor-pointer items-center gap-2.5 rounded-lg px-3 text-[13px] transition-[background-color,opacity] duration-100 select-none data-active:bg-accent aria-disabled:cursor-default aria-disabled:not-aria-busy:opacity-50"
    >
      <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {busy ? (
        <LoaderCircle aria-label="Working" className="size-3.5 shrink-0 animate-spin text-muted-foreground" />
      ) : (
        hint && <span className="shrink-0 text-[11.5px] text-muted-foreground">{hint}</span>
      )}
    </li>
  );
}
