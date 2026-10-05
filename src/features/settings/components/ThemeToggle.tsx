import { Monitor, Moon, Sun, type LucideIcon } from "lucide-react";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { Theme } from "@/lib/bindings";
import { usePressScale } from "@/shared";
import type { ControlIds } from "./SettingRow";

const THEMES: readonly { value: Theme; label: string; icon: LucideIcon }[] = [
  { value: "system", label: "System", icon: Monitor },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
];

/** System / light / dark as a compact icon segment. Arrow keys move between them. */
export function ThemeToggle({ value, onChange, ids }: { value: Theme; onChange: (theme: Theme) => void; ids: ControlIds }) {
  return (
    <ToggleGroup
      value={[value]}
      onValueChange={(next) => {
        // Pressing the selected one again would empty the group: keep it selected.
        if (next[0]) onChange(next[0] as Theme);
      }}
      spacing={0}
      aria-labelledby={ids.labelId}
      className="gap-0.5 rounded-lg bg-muted p-0.5"
    >
      {THEMES.map((theme) => (
        <ThemeItem key={theme.value} {...theme} />
      ))}
    </ToggleGroup>
  );
}

function ThemeItem({ value, label, icon: Icon }: (typeof THEMES)[number]) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.92 });
  return (
    <ToggleGroupItem
      ref={ref}
      value={value}
      aria-label={label}
      title={label}
      className="size-7 min-w-7 rounded-md! px-0! text-muted-foreground hover:bg-transparent hover:text-foreground aria-pressed:bg-background aria-pressed:text-foreground aria-pressed:shadow-sm"
    >
      <Icon className="size-4" strokeWidth={1.75} />
    </ToggleGroupItem>
  );
}
