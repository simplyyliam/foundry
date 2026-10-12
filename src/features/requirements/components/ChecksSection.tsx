import { useState, type KeyboardEvent } from "react";
import { Check, Plus, ShieldCheck, Sparkles, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { PlanRequirement, RequirementCheck } from "@/lib/bindings";
import { CHECK_TYPES, PressButton, buildCheck, checkKey, describeCheck, type CheckField, type CheckType } from "@/shared";
import { useAction } from "../hooks/useAction";
import { useDismissedChecks } from "../hooks/useDismissedChecks";
import type { RequirementActions } from "../hooks/useRequirementActions";
import { ActionError } from "./Notices";

const FIELDS: Record<CheckField, { label: string; placeholder: string }> = {
  path: { label: "Path", placeholder: "components.json" },
  name: { label: "Package", placeholder: "@radix-ui/*" },
  glob: { label: "Files", placeholder: "src/**/*.tsx" },
  pattern: { label: "Pattern (regex)", placeholder: "from \"@radix-ui/" },
};

interface ChecksSectionProps {
  requirement: PlanRequirement;
  actions: RequirementActions;
}

/**
 * The requirement's checks, with remove, "Add check" and its suggested checks. Unlike the
 * dialog's text fields, every change saves at once (the list always shows what's saved).
 */
export function ChecksSection({ requirement, actions }: ChecksSectionProps) {
  const checks = requirement.checks ?? [];
  const { busy, error, run } = useAction();
  const { isDismissed, dismiss } = useDismissedChecks(actions.projectId, requirement.id);
  const [adding, setAdding] = useState(false);
  // Indices into the full `suggestedChecks`: that's what accept_requirement_check takes.
  const suggestions = (requirement.suggestedChecks ?? [])
    .map((check, index) => ({ check, index }))
    .filter(({ check }) => !isDismissed(check));

  const save = (next: RequirementCheck[]) => run(() => actions.setChecks(requirement.id, next));

  return (
    <section aria-labelledby="requirement-checks" className="flex flex-col gap-2">
      <div className="flex flex-col gap-1">
        <h3 id="requirement-checks" className="text-[13px] leading-none font-medium">
          Checks
        </h3>
        <p className="text-[12px] text-muted-foreground">Verify runs these itself, after the test commands. Changes save right away.</p>
      </div>

      {checks.length === 0 ? (
        <p className="text-[12.5px] text-muted-foreground">No checks yet.</p>
      ) : (
        <ul aria-label="Checks" className="flex flex-col gap-1">
          {checks.map((check, index) => (
            <li key={`${checkKey(check)}-${index}`} className="flex items-center gap-2 rounded-lg bg-muted/50 py-1 pr-1 pl-2.5">
              <ShieldCheck aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 text-[12.5px] leading-snug break-words select-text">{describeCheck(check)}</span>
              <PressButton
                type="button"
                variant="ghost"
                size="icon-xs"
                pressScale={0.9}
                disabled={busy}
                aria-label={`Remove check: ${describeCheck(check)}`}
                onClick={() => void save(checks.filter((_, other) => other !== index))}
                className="rounded-full text-muted-foreground"
              >
                <X aria-hidden />
              </PressButton>
            </li>
          ))}
        </ul>
      )}

      {adding ? (
        <AddCheckForm
          busy={busy}
          onCancel={() => setAdding(false)}
          onAdd={async (check) => {
            if ((await save([...checks, check])).ok) setAdding(false);
          }}
        />
      ) : (
        <PressButton type="button" variant="ghost" size="xs" onClick={() => setAdding(true)} className="w-fit text-muted-foreground">
          <Plus aria-hidden data-icon="inline-start" />
          Add check
        </PressButton>
      )}

      {suggestions.length > 0 && (
        <div role="group" aria-label="Suggested checks" className="flex flex-col gap-1.5 pt-1">
          <span className="flex items-center gap-1.5 text-[12px] text-muted-foreground">
            <Sparkles aria-hidden className="size-3.5" />
            Suggested from the roadmap
          </span>
          <ul className="flex flex-wrap gap-1.5">
            {suggestions.map(({ check, index }) => (
              <li
                key={`${checkKey(check)}-${index}`}
                className="flex max-w-full items-center gap-1 rounded-full border border-dashed border-border py-0.5 pr-0.5 pl-2.5"
              >
                <span className="min-w-0 text-[12px] leading-snug break-words">{describeCheck(check)}</span>
                <PressButton
                  type="button"
                  variant="ghost"
                  size="xs"
                  pressScale={0.94}
                  disabled={busy}
                  aria-label={`Accept: ${describeCheck(check)}`}
                  onClick={() => void run(() => actions.acceptCheck(requirement.id, index))}
                  className="h-6 rounded-full px-2"
                >
                  <Check aria-hidden data-icon="inline-start" />
                  Accept
                </PressButton>
                <PressButton
                  type="button"
                  variant="ghost"
                  size="xs"
                  pressScale={0.94}
                  aria-label={`Dismiss: ${describeCheck(check)}`}
                  onClick={() => dismiss(check)}
                  className="h-6 rounded-full px-2 text-muted-foreground"
                >
                  Dismiss
                </PressButton>
              </li>
            ))}
          </ul>
        </div>
      )}

      <ActionError error={error} />
    </section>
  );
}

function AddCheckForm({
  busy,
  onAdd,
  onCancel,
}: {
  busy: boolean;
  onAdd: (check: RequirementCheck) => void;
  onCancel: () => void;
}) {
  const [type, setType] = useState<CheckType>("file_exists");
  const [values, setValues] = useState<Partial<Record<CheckField, string>>>({});
  const fields = CHECK_TYPES.find((option) => option.type === type)?.fields ?? [];
  const check = buildCheck(type, values);

  // Inside the dialog's form: Enter adds the check instead of saving the requirement.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Enter" || event.metaKey || event.ctrlKey) return;
    event.preventDefault();
    if (check && !busy) onAdd(check);
  };

  return (
    <div role="group" aria-label="New check" className="flex flex-col gap-2.5 rounded-xl p-3 ring-1 ring-foreground/8">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="check-type" className="text-[12px]">
          Type
        </Label>
        <Select
          value={type}
          items={CHECK_TYPES.map((option) => ({ value: option.type, label: option.label }))}
          onValueChange={(next) => {
            if (typeof next === "string") setType(next as CheckType);
          }}
        >
          <SelectTrigger id="check-type" size="sm" className="w-full text-[13px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent alignItemWithTrigger={false}>
            {CHECK_TYPES.map((option) => (
              <SelectItem key={option.type} value={option.type} className="text-[13px]">
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="flex flex-wrap gap-2.5">
        {fields.map((field, position) => (
          <div key={field} className="flex min-w-40 flex-1 flex-col gap-1.5">
            <Label htmlFor={`check-${field}`} className="text-[12px]">
              {FIELDS[field].label}
            </Label>
            <Input
              id={`check-${field}`}
              autoFocus={position === 0}
              value={values[field] ?? ""}
              onChange={(event) => setValues((previous) => ({ ...previous, [field]: event.target.value }))}
              onKeyDown={onKeyDown}
              placeholder={FIELDS[field].placeholder}
              spellCheck={false}
              className="h-8 font-mono text-[12.5px]"
            />
          </div>
        ))}
      </div>
      {check && <p className="text-[12px] text-muted-foreground">Adds: {describeCheck(check)}</p>}
      <div className="flex justify-end gap-1.5">
        <PressButton type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </PressButton>
        <PressButton type="button" variant="secondary" size="sm" disabled={busy || !check} onClick={() => check && onAdd(check)}>
          Add
        </PressButton>
      </div>
    </div>
  );
}
