import { useState, type KeyboardEvent, type ReactNode } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { Outcome } from "../hooks/call";

interface RoleFieldProps {
  id: string;
  value: string;
  placeholder?: string;
  multiline?: boolean;
  className?: string;
  /** Saved mode: called on blur (or Enter for one line) when the text changed. */
  onCommit?: (text: string) => Promise<Outcome<unknown>>;
  /** Draft mode (a new role): every keystroke. */
  onChange?: (text: string) => void;
  describedBy?: string;
  /** Shown under the field, from the text as currently typed (not just as saved). */
  below?: (text: string) => ReactNode;
}

/**
 * A role's text field. Saved mode saves on blur or Enter (Ctrl+Enter in a textarea), never
 * per keystroke; Escape puts back the saved text; a failed save goes back to it and shows
 * the backend's message under the field.
 */
export function RoleField({ id, value, placeholder, multiline, className, onCommit, onChange, describedBy, below }: RoleFieldProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const commit = async () => {
    if (!onCommit || draft === null) return;
    const text = draft.trim();
    setDraft(null);
    if (text === value) return;
    setPending(text);
    setError(null);
    const outcome = await onCommit(text);
    setPending(null);
    if (!outcome.ok) setError(outcome.error.message);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    if (!onCommit) return;
    if (event.key === "Escape" && draft !== null && draft !== value) {
      event.stopPropagation();
      setDraft(null);
    } else if (event.key === "Enter" && (!multiline || event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void commit();
    }
  };

  const errorId = error ? `${id}-error` : undefined;
  const shown = onCommit ? (draft ?? pending ?? value) : value;
  const props = {
    id,
    value: shown,
    placeholder,
    onChange: (event: { target: { value: string } }) => (onCommit ? setDraft(event.target.value) : onChange?.(event.target.value)),
    onBlur: () => void commit(),
    onKeyDown,
    autoComplete: "off",
    "aria-invalid": error ? true : undefined,
    "aria-describedby": [describedBy, errorId].filter(Boolean).join(" ") || undefined,
  };

  return (
    <>
      {multiline ? (
        <Textarea {...props} className={cn("text-[13px] md:text-[13px]", className)} />
      ) : (
        <Input {...props} className={cn("h-9 rounded-lg text-[13px] md:text-[13px]", className)} />
      )}
      {below?.(shown)}
      {error && (
        <p id={errorId} role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </>
  );
}
