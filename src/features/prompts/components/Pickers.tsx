import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { SliceView, TeamAgent } from "@/lib/bindings";
import { SliceStatusChip } from "./SliceStatusChip";

interface PickerProps<T> {
  id: string;
  options: readonly T[];
  value: string | null;
  onChange: (id: string) => void;
  disabled?: boolean;
  /** Shown while nothing is picked. */
  placeholder: string;
}

/** The slices in layer order, each with its status chip. */
export function SlicePicker({ id, options, value, onChange, disabled, placeholder }: PickerProps<SliceView>) {
  const picked = options.find((slice) => slice.id === value) ?? null;
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <Label htmlFor={id} className="text-[12.5px] text-muted-foreground">
        Slice
      </Label>
      <Select
        value={picked?.id ?? null}
        items={options.map((slice) => ({ value: slice.id, label: slice.name }))}
        disabled={disabled}
        onValueChange={(next) => {
          if (typeof next === "string") onChange(next);
        }}
      >
        <SelectTrigger id={id} className="w-full text-[13px]">
          <SelectValue>
            {() =>
              picked ? (
                <span className="flex min-w-0 items-center gap-2">
                  <span className="truncate">{picked.name}</span>
                  <SliceStatusChip status={picked.status} />
                </span>
              ) : (
                <span className="text-muted-foreground">{placeholder}</span>
              )
            }
          </SelectValue>
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false} className="min-w-64">
          {options.map((slice) => (
            <SelectItem key={slice.id} value={slice.id} className="text-[13px]">
              <span className="min-w-0 truncate">{slice.name}</span>
              <SliceStatusChip status={slice.status} className="ml-auto" />
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

/** The confirmed team's agents: display name, then title. */
export function AgentPicker({ id, options, value, onChange, disabled, placeholder }: PickerProps<TeamAgent>) {
  const picked = options.find((agent) => agent.id === value) ?? null;
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <Label htmlFor={id} className="text-[12.5px] text-muted-foreground">
        Agent
      </Label>
      <Select
        value={picked?.id ?? null}
        items={options.map((agent) => ({ value: agent.id, label: agent.displayName }))}
        disabled={disabled}
        onValueChange={(next) => {
          if (typeof next === "string") onChange(next);
        }}
      >
        <SelectTrigger id={id} className="w-full text-[13px]">
          <SelectValue>
            {() =>
              picked ? (
                <span className="flex min-w-0 items-baseline gap-1.5">
                  <span className="truncate">{picked.displayName}</span>
                  <span className="truncate text-[12px] text-muted-foreground">{picked.title}</span>
                </span>
              ) : (
                <span className="text-muted-foreground">{placeholder}</span>
              )
            }
          </SelectValue>
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false} className="min-w-64">
          {options.map((agent) => (
            <SelectItem key={agent.id} value={agent.id} className="text-[13px]">
              <span className="min-w-0 truncate">{agent.displayName}</span>
              <span className="ml-auto truncate pl-3 text-[12px] font-normal text-muted-foreground">{agent.title}</span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
