import { useId, type ReactNode } from "react";
import { cn } from "@/lib/utils";

export interface ControlIds {
  /** For the control's `aria-labelledby`. */
  labelId: string;
  /** For the control's `aria-describedby` (description and error). */
  describedBy: string | undefined;
}

interface SettingRowProps {
  label: ReactNode;
  description?: ReactNode;
  /** Beside the label, e.g. `<NotActiveTag />`. */
  tag?: ReactNode;
  /** The control on the right. */
  control?: ReactNode | ((ids: ControlIds) => ReactNode);
  /** The backend's message from a failed save, shown under the row as-is. */
  error?: string | null;
  className?: string;
}

/** One setting: label and description on the left, its control on the right. */
export function SettingRow({ label, description, tag, control, error, className }: SettingRowProps) {
  const id = useId();
  const labelId = `${id}-label`;
  const descriptionId = description ? `${id}-description` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [descriptionId, errorId].filter(Boolean).join(" ") || undefined;

  return (
    <div className={cn("flex flex-col gap-1.5 border-b border-border/60 py-3.5 last:border-b-0", className)}>
      <div className="flex min-h-8 items-center justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <span id={labelId} className="text-[13px] font-medium">
              {label}
            </span>
            {tag}
          </div>
          {description && (
            <div id={descriptionId} className="min-w-0 text-[12.5px] text-muted-foreground">
              {description}
            </div>
          )}
        </div>
        {control && (
          <div className="flex shrink-0 items-center gap-1.5">
            {typeof control === "function" ? control({ labelId, describedBy }) : control}
          </div>
        )}
      </div>
      {error && (
        <p id={errorId} role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

/** A titled group of rows inside a section. */
export function SettingGroup({ title, action, children }: { title?: ReactNode; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col">
      {(title || action) && (
        <div className="flex min-h-8 items-center justify-between gap-4 pt-4">
          {title && <h3 className="text-[13px] font-semibold">{title}</h3>}
          {action}
        </div>
      )}
      <div className="flex flex-col">{children}</div>
    </section>
  );
}
