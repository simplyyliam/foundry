import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { usePressScale } from "@/shared";

interface LocationChipProps {
  icon: LucideIcon;
  label: string;
  active: boolean;
  onClick: () => void;
}

export function LocationChip({ icon: Icon, label, active, onClick }: LocationChipProps) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.96 });

  return (
    <button
      ref={ref}
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "flex h-7 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-[12px] font-medium outline-none",
        "text-muted-foreground transition-colors duration-150 hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/40",
        active && "bg-accent text-foreground",
      )}
    >
      <Icon aria-hidden className="size-3.5" strokeWidth={1.75} />
      {label}
    </button>
  );
}
