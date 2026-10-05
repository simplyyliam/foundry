import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { ControlIds } from "./SettingRow";

interface SettingSelectProps<T extends string> {
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
  ids: ControlIds;
  disabled?: boolean;
}

/** A borderless select on the right of a row: the current choice and a chevron. */
export function SettingSelect<T extends string>({ value, options, onChange, ids, disabled }: SettingSelectProps<T>) {
  return (
    <Select
      value={value}
      items={options}
      disabled={disabled}
      onValueChange={(next) => {
        if (next !== null) onChange(next as T);
      }}
    >
      <SelectTrigger
        size="sm"
        aria-labelledby={ids.labelId}
        aria-describedby={ids.describedBy}
        className="h-8 rounded-md bg-transparent pr-2 pl-2.5 text-[13px] hover:bg-accent data-popup-open:bg-accent"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent align="end" alignItemWithTrigger={false} className="min-w-44">
        {options.map((option) => (
          <SelectItem key={option.value} value={option.value} className="text-[13px]">
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
