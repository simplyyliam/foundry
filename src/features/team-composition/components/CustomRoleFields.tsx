import { useId } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { CustomRole } from "@/lib/bindings";

/** Title, description and skill text of a role that isn't in the handbook. */
export function CustomRoleFields({
  value,
  onChange,
  autoFocus,
}: {
  value: CustomRole;
  onChange: (value: CustomRole) => void;
  autoFocus?: boolean;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${id}-title`} className="text-[12.5px]">
          Title
        </Label>
        <Input
          id={`${id}-title`}
          value={value.title}
          onChange={(event) => onChange({ ...value, title: event.target.value })}
          placeholder="e.g. Data Engineer"
          autoFocus={autoFocus}
          className="text-[13px]"
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${id}-description`} className="text-[12.5px]">
          Description
        </Label>
        <Textarea
          id={`${id}-description`}
          value={value.description}
          onChange={(event) => onChange({ ...value, description: event.target.value })}
          placeholder="What this role is responsible for"
          className="min-h-14 py-2 text-[13px]"
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${id}-skill`} className="text-[12.5px]">
          Skill
        </Label>
        <Textarea
          id={`${id}-skill`}
          value={value.skillTemplate}
          onChange={(event) => onChange({ ...value, skillTemplate: event.target.value })}
          placeholder="Who they are and how they work. Goes into the agent's role file."
          className="max-h-56 min-h-24 py-2 font-mono text-[12.5px]"
        />
      </div>
    </div>
  );
}
