import { useState, type ComponentProps, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";

interface CommitInputProps extends Omit<ComponentProps<typeof Input>, "value" | "defaultValue" | "onChange"> {
  /** The value to show while not editing (the field's current value, as text). */
  value: string;
  /** Called on blur or Enter when the text changed. */
  onCommit: (text: string) => void;
}

/**
 * A text field that saves on blur or Enter, never per keystroke. Escape puts back the shown
 * value without saving (and only closes the dialog if there was nothing to undo).
 */
export function CommitInput({ value, onCommit, onBlur, onKeyDown, ...props }: CommitInputProps) {
  // null while not editing, so the field follows `value` (including a revert after a failed save).
  const [draft, setDraft] = useState<string | null>(null);

  const commit = () => {
    if (draft === null) return;
    setDraft(null);
    if (draft !== value) onCommit(draft);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    onKeyDown?.(event);
    if (event.key === "Enter") {
      event.preventDefault();
      commit();
    } else if (event.key === "Escape" && draft !== null && draft !== value) {
      event.stopPropagation();
      setDraft(null);
    }
  };

  return (
    <Input
      {...props}
      value={draft ?? value}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={(event) => {
        onBlur?.(event);
        commit();
      }}
      onKeyDown={handleKeyDown}
      autoComplete="off"
      spellCheck={false}
    />
  );
}
