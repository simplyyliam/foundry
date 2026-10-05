import { ChevronDown } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import type { SliceView } from "@/lib/bindings";
import { PressButton } from "@/shared";

const NONE = "__unassigned__";

interface SliceMenuProps {
  /** In Blueprint order. */
  slices: SliceView[];
  value: string | null;
  onChange: (sliceId: string | null) => void;
  disabled?: boolean;
  /** Unassigning a confirmed requirement makes it proposed again: say so in the menu. */
  unassignResets?: boolean;
  className?: string;
}

/** Pick a requirement's slice, or Unassigned. */
export function SliceMenu({ slices, value, onChange, disabled, unassignResets, className }: SliceMenuProps) {
  const current = slices.find((slice) => slice.id === value);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <PressButton
            variant="outline"
            size="xs"
            disabled={disabled}
            aria-label={`Slice: ${current?.name ?? "Unassigned"}`}
            className={cn("w-44 justify-between", !current && "text-muted-foreground", className)}
          >
            <span className="truncate">{current?.name ?? "Unassigned"}</span>
            <ChevronDown aria-hidden data-icon="inline-end" />
          </PressButton>
        }
      />
      <DropdownMenuContent align="end" className="w-64">
        <DropdownMenuRadioGroup
          value={current ? current.id : NONE}
          onValueChange={(next: string) => {
            const sliceId = next === NONE ? null : next;
            if (sliceId !== (current?.id ?? null)) onChange(sliceId);
          }}
        >
          {slices.map((slice) => (
            <DropdownMenuRadioItem key={slice.id} value={slice.id}>
              <span className="truncate">{slice.name}</span>
            </DropdownMenuRadioItem>
          ))}
          {slices.length > 0 && <DropdownMenuSeparator />}
          <DropdownMenuRadioItem value={NONE}>
            <span className="flex flex-col">
              <span>Unassigned</span>
              {unassignResets && <span className="text-[11px] font-normal text-muted-foreground">Moves it back to proposed</span>}
            </span>
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
