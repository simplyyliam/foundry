import { Switch } from "@/components/ui/switch";
import { usePressScale } from "@/shared";
import type { ControlIds } from "./SettingRow";

/** A row's on/off switch. Saves the moment it's flipped. */
export function SettingSwitch({ checked, onChange, ids }: { checked: boolean; onChange: (checked: boolean) => void; ids: ControlIds }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.94 });
  return (
    <Switch
      ref={ref}
      checked={checked}
      onCheckedChange={onChange}
      aria-labelledby={ids.labelId}
      aria-describedby={ids.describedBy}
    />
  );
}
