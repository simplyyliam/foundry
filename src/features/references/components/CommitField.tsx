import { useId, useRef, useState, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { AppError } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { Outcome } from "../hooks/call";

interface CommitFieldProps {
  value: string;
  /** Accessible name. */
  label: string;
  placeholder?: string;
  multiline?: boolean;
  className?: string;
  /** A muted line under the field, linked as its description. */
  hint?: string;
  /** Asked (one line, Save anyway / Keep editing) before saving the field empty over saved text. */
  confirmEmpty?: string;
  /** Saves the new text; the field shows the backend's error if it fails. */
  onCommit: (text: string) => Promise<Outcome<unknown>>;
}

/**
 * A field that saves on blur or Enter (Shift+Enter is a newline when multiline), never per
 * keystroke. Enter also moves to the next reference field, so a batch can be labelled from
 * the keyboard. Escape puts back the saved text.
 */
export function CommitField({ value, label, placeholder, multiline, className, hint, confirmEmpty, onCommit }: CommitFieldProps) {
  const [draft, setDraft] = useState(value);
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [confirming, setConfirming] = useState(false);
  const fieldRef = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  const hintId = useId();
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
    if (confirmEmpty && draft.trim() === "") return setConfirming(true);
    await save(draft.trim());
  };

  const save = async (text: string) => {
    setConfirming(false);
    const outcome = await onCommit(text);
    setError(outcome.ok ? null : outcome.error);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      setDraft(value);
      setError(null);
      setConfirming(false);
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
    "aria-describedby": hint ? hintId : undefined,
    ref: fieldRef,
    value: draft,
    placeholder,
    onFocus: () => {
      setEditing(true);
      // Back in the field: the question waits for the next blur.
      setConfirming(false);
    },
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
      {hint && (
        <p id={hintId} className="text-[11.5px] leading-snug text-muted-foreground">
          {hint}
        </p>
      )}
      {confirming && confirmEmpty && (
        <div role="alert" className="flex flex-wrap items-center gap-x-2 gap-y-1 rounded-lg bg-muted/70 py-1 pr-1 pl-2.5 text-[11.5px]">
          <p className="mr-auto leading-snug">{confirmEmpty}</p>
          <span className="flex gap-1">
            <PressButton size="xs" variant="ghost" className="h-6" onClick={() => fieldRef.current?.focus()}>
              Keep editing
            </PressButton>
            <PressButton size="xs" variant="secondary" className="h-6" onClick={() => void save("")}>
              Save anyway
            </PressButton>
          </span>
        </div>
      )}
      {error && (
        <p role="alert" className="text-[11px] leading-snug text-destructive">
          {error.message}
        </p>
      )}
    </div>
  );
}
