import { useId, useState } from "react";
import { Switch } from "@/components/ui/switch";
import type { SliceView } from "@/lib/bindings";
import { usePressScale } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { PlanActions } from "../hooks/usePlanActions";
import { ActionError } from "./Notices";

const DESCRIPTION =
  "Lets the agent for this slice edit package.json, pnpm-workspace.yaml, pnpm-lock.yaml, tsconfig.base.json, .gitignore and .npmrc. Use it for the project foundation slice.";
const LOCKED = "Locked: this slice is done.";
const NEXT_START = "Takes effect when an agent next starts on this slice";

/**
 * The slice's `workspaceAccess`. Saves the moment it's flipped (showing the new value while
 * saving). It applies at the next launch, so an in-progress slice can still change it (an
 * agent already running keeps the rules it launched with); only a done slice is locked.
 */
export function SliceWorkspaceAccess({ slice, actions }: { slice: SliceView; actions: PlanActions }) {
  const { busy, error, run } = useAction();
  const [pending, setPending] = useState<boolean | null>(null);
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.94 });
  const id = useId();
  const locked = slice.status === "done";
  const note = locked ? LOCKED : slice.status === "in_progress" ? NEXT_START : null;
  const checked = pending ?? slice.workspaceAccess ?? false;

  const change = async (value: boolean) => {
    if (busy || locked) return;
    setPending(value);
    await run(() => actions.setWorkspaceAccess(slice.id, value));
    setPending(null);
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-start justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <span id={`${id}-label`} className="text-[13px] font-medium">
            Allow root files and lockfile
          </span>
          <p id={`${id}-description`} className="text-[12px] leading-snug text-muted-foreground">
            {DESCRIPTION}
          </p>
          {note && (
            <p id={`${id}-note`} className="text-[12px] leading-snug text-muted-foreground">
              {note}
            </p>
          )}
        </div>
        <Switch
          ref={ref}
          className="mt-0.5"
          checked={checked}
          disabled={locked || busy}
          onCheckedChange={(value) => void change(value)}
          aria-labelledby={`${id}-label`}
          aria-describedby={note ? `${id}-description ${id}-note` : `${id}-description`}
        />
      </div>
      <ActionError error={error} />
    </div>
  );
}
