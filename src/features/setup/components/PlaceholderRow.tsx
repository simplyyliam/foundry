import { Check, Pencil } from "lucide-react";
import { useId, useState, type KeyboardEvent } from "react";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import type { Placeholder } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { Outcome } from "../hooks/call";

interface PlaceholderRowProps {
  placeholder: Placeholder;
  /** The text being typed, or null when a confirmed value isn't being edited. */
  draft: string | null;
  onDraftChange: (text: string | null) => void;
  onConfirm: (text: string) => Promise<Outcome<unknown>>;
  disabled: boolean;
}

/**
 * One AGENTS.md placeholder. Unconfirmed: a growing textarea and Confirm (Enter; Shift+Enter
 * for a new line). Confirmed: its value, a check and Edit; editing makes it unconfirmed
 * until confirmed again, and Escape or Cancel puts the confirmed value back.
 */
export function PlaceholderRow({ placeholder, draft, onDraftChange, onConfirm, disabled }: PlaceholderRowProps) {
  const id = useId();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editing = !placeholder.confirmed || draft !== null;
  const text = draft ?? placeholder.value ?? "";
  // An optional placeholder can be confirmed empty (that removes its line from the file).
  const canConfirm = !disabled && !busy && (placeholder.optional || text.trim() !== "");

  const confirm = async () => {
    if (!canConfirm) return;
    setBusy(true);
    setError(null);
    const outcome = await onConfirm(text);
    setBusy(false);
    // folder_missing shows once, in the section's banner.
    if (!outcome.ok && outcome.error.code !== "folder_missing") setError(outcome.error.message);
  };

  const cancel = () => {
    setError(null);
    onDraftChange(null);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void confirm();
    } else if (event.key === "Escape" && placeholder.confirmed && draft !== null) {
      event.preventDefault();
      cancel();
    }
  };

  const hintId = placeholder.hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;

  return (
    <li className="flex flex-col gap-2 border-b border-border/60 py-4 first:pt-1 last:border-b-0">
      <div className="flex min-h-7 items-center gap-2">
        <label htmlFor={`${id}-input`} id={`${id}-label`} className="text-[13px] font-medium">
          {placeholder.label}
        </label>
        {placeholder.optional && (
          <Badge variant="secondary" className="h-5 rounded-md px-1.5 text-[11px] font-normal text-muted-foreground">
            Optional
          </Badge>
        )}
        {!editing && (
          <>
            <span className="ml-auto flex items-center gap-1 text-[12px] text-muted-foreground">
              <Check aria-hidden className="size-3.5 text-primary" />
              Confirmed
            </span>
            <PressButton
              variant="ghost"
              size="xs"
              disabled={disabled}
              aria-label={`Edit ${placeholder.label}`}
              onClick={() => onDraftChange(placeholder.value ?? "")}
              className="rounded-md text-muted-foreground"
            >
              <Pencil data-icon="inline-start" />
              Edit
            </PressButton>
          </>
        )}
      </div>
      {placeholder.hint && (
        <p id={hintId} className="-mt-1 text-[12px] leading-snug text-muted-foreground">
          {placeholder.hint}
        </p>
      )}

      {editing ? (
        <>
          <Textarea
            id={`${id}-input`}
            data-placeholder-input={placeholder.key}
            value={text}
            rows={1}
            placeholder={placeholder.hint || undefined}
            disabled={disabled}
            // Only when Edit was clicked: an unconfirmed list shouldn't grab focus on load.
            autoFocus={placeholder.confirmed}
            onFocus={(event) => {
              const end = event.currentTarget.value.length;
              event.currentTarget.setSelectionRange(end, end);
            }}
            onChange={(event) => onDraftChange(event.target.value)}
            onKeyDown={onKeyDown}
            autoComplete="off"
            aria-invalid={error ? true : undefined}
            aria-describedby={[hintId, errorId].filter(Boolean).join(" ") || undefined}
            className="min-h-9 px-3 py-2 text-[13px] md:text-[13px]"
          />
          <div className="flex items-center justify-end gap-2">
            {placeholder.confirmed && (
              <PressButton variant="ghost" size="sm" disabled={busy} onClick={cancel}>
                Cancel
              </PressButton>
            )}
            <PressButton size="sm" disabled={!canConfirm} onClick={() => void confirm()}>
              Confirm
            </PressButton>
          </div>
        </>
      ) : (
        <p className="rounded-lg bg-muted/60 px-3 py-2 text-[13px] whitespace-pre-wrap">
          {placeholder.value ? placeholder.value : <span className="text-muted-foreground italic">Empty</span>}
        </p>
      )}

      {error && (
        <p id={errorId} role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </li>
  );
}
