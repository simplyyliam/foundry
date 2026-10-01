import type { LucideIcon } from "lucide-react";
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
  onHover: () => void;
  onChoose: () => void;
}

/** One row in the search results. Rows aren't focusable: the search field keeps focus. */
export function ProjectOption({ id, icon: Icon, label, hint, active, onHover, onChoose }: ProjectOptionProps) {
  const ref = usePressScale<HTMLLIElement>({ scale: 0.985 });

  return (
    <li
      ref={ref}
      id={id}
      role="option"
      aria-selected={active}
      data-active={active || undefined}
      onPointerMove={onHover}
      // Keep focus in the search field.
      onPointerDown={(event) => event.preventDefault()}
      onClick={onChoose}
      className="flex h-9 cursor-pointer items-center gap-2.5 rounded-lg px-3 text-[13px] transition-colors duration-100 select-none data-active:bg-accent"
    >
      <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {hint && <span className="shrink-0 text-[11.5px] text-muted-foreground">{hint}</span>}
    </li>
  );
}
