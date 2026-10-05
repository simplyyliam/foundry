import { useRef, useState, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { AppError } from "@/lib/bindings";
import type { Outcome } from "../hooks/call";

interface CommitFieldProps {
  value: string;
  /** Accessible name. */
  label: string;
  placeholder?: string;
  multiline?: boolean;
  className?: string;
  /** Saves the new text; the field shows the backend's error if it fails. */
  onCommit: (text: string) => Promise<Outcome<unknown>>;
}

/**
 * A field that saves on blur or Enter (Shift+Enter is a newline when multiline), never per
 * keystroke. Enter also moves to the next reference field, so a batch can be labelled from
 * the keyboard. Escape puts back the saved text.
 */
export function CommitField({ value, label, placeholder, multiline, className, onCommit }: CommitFieldProps) {
  const [draft, setDraft] = useState(value);
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  // Escape blurs the field; that blur must not save the text it just discarded.
  const discarding = useRef(false);
  // Follow the saved value (e.g. after a save from the sheet) unless the user is typing.
  const [lastValue, setLastValue] = useState(value);
  if (value !== lastValue) {
    setLastValue(value);
    if (!editing) setDraft(value);
  }

  const commit = async () => {
    setEditing(false);
    if (discarding.current) {
      discarding.current = false;
      return;
    }
    if (draft.trim() === value.trim()) return setDraft(value);
    const outcome = await onCommit(draft.trim());
    setError(outcome.ok ? null : outcome.error);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      setDraft(value);
      setError(null);
      discarding.current = true;
      event.currentTarget.blur();
    } else if (event.key === "Enter" && !(multiline && event.shiftKey)) {
      event.preventDefault();
      // Moving focus blurs this field, which saves it.
      const fields = Array.from(
        event.currentTarget.closest("[data-references-root]")?.querySelectorAll<HTMLElement>("[data-reference-field]") ?? [],
      );
      const next = fields[fields.indexOf(event.currentTarget) + 1];
      if (next) next.focus();
      else event.currentTarget.blur();
    }
  };

  const shared = {
    "data-reference-field": "",
    "aria-label": label,
    "aria-invalid": error ? true : undefined,
    value: draft,
    placeholder,
    onFocus: () => setEditing(true),
    onChange: (event: { target: { value: string } }) => setDraft(event.target.value),
    onBlur: () => void commit(),
    onKeyDown,
  };

  return (
    <div className="flex flex-col gap-1">
      {multiline ? (
        <Textarea {...shared} className={cn("min-h-9 px-2.5 py-1.5 text-[12px] md:text-[12px]", className)} />
      ) : (
        <Input {...shared} className={cn("h-7 px-2.5 text-[12px] md:text-[12px]", className)} />
      )}
      {error && (
        <p role="alert" className="text-[11px] leading-snug text-destructive">
          {error.message}
        </p>
      )}
    </div>
  );
}
