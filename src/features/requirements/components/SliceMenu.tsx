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
const PROJECT = "__project_wide__";

interface SliceMenuProps {
  /** In Blueprint order. */
  slices: SliceView[];
  value: string | null;
  onChange: (sliceId: string | null) => void;
  disabled?: boolean;
  /** Unassigning a confirmed requirement makes it proposed again: say so in the menu. */
  unassignResets?: boolean;
  /** A project rule: the menu reads "Project-wide", and any other pick turns that off. */
  projectWide?: boolean;
  className?: string;
}

/** Pick a requirement's slice, or Unassigned. */
export function SliceMenu({ slices, value, onChange, disabled, unassignResets, projectWide, className }: SliceMenuProps) {
  const current = projectWide ? undefined : slices.find((slice) => slice.id === value);
  const label = current?.name ?? (projectWide ? "Project-wide" : "Unassigned");

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <PressButton
            variant="outline"
            size="xs"
            disabled={disabled}
            aria-label={`Slice: ${label}`}
            className={cn("w-44 justify-between", !current && !projectWide && "text-muted-foreground", className)}
          >
            <span className="truncate">{label}</span>
            <ChevronDown aria-hidden data-icon="inline-end" />
          </PressButton>
        }
      />
      <DropdownMenuContent align="end" className="w-64">
        <DropdownMenuRadioGroup
          value={current ? current.id : projectWide ? PROJECT : NONE}
          onValueChange={(next: string) => {
            if (next === PROJECT) return;
            const sliceId = next === NONE ? null : next;
            if (projectWide || sliceId !== (current?.id ?? null)) onChange(sliceId);
          }}
        >
          {slices.map((slice) => (
            <DropdownMenuRadioItem key={slice.id} value={slice.id}>
              <span className="truncate">{slice.name}</span>
            </DropdownMenuRadioItem>
          ))}
          {slices.length > 0 && <DropdownMenuSeparator />}
          {projectWide && (
            <DropdownMenuRadioItem value={PROJECT}>
              <span>Project-wide</span>
            </DropdownMenuRadioItem>
          )}
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
