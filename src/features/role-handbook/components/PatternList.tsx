import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState, type FocusEvent, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import { PressButton } from "@/shared";
import type { Outcome } from "../hooks/call";

interface PatternListProps {
  /** Accessible name for the list, e.g. "Write patterns". */
  label: string;
  patterns: readonly string[];
  placeholder: string;
  /**
   * Saved mode: called when focus leaves the list or a row is removed, with the trimmed,
   * non-empty rows. On an error the rows go back to `patterns`.
   */
  onCommit?: (patterns: string[]) => Promise<Outcome<unknown>>;
  /** Draft mode (a new role): every edit, rows as typed. */
  onChange?: (rows: string[]) => void;
}

const clean = (rows: readonly string[]) => rows.map((row) => row.trim()).filter(Boolean);
const equal = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((value, i) => value === b[i]);

/** Glob patterns as text rows. Enter adds a row below; empty rows are dropped when saved. */
export function PatternList({ label, patterns, placeholder, onCommit, onChange }: PatternListProps) {
  const [draft, setDraft] = useState<string[] | null>(null);
  const [pending, setPending] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  // A row to focus after the next render (a row added or removed by the keyboard).
  const focusRow = useRef<number | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const rows = onCommit ? (draft ?? pending ?? [...patterns]) : [...patterns];

  useEffect(() => {
    if (focusRow.current === null) return;
    listRef.current?.querySelectorAll<HTMLInputElement>("[data-pattern-row]")[focusRow.current]?.focus();
    focusRow.current = null;
  });

  const save = async (next: string[]) => {
    if (!onCommit) return;
    setDraft(null);
    const cleaned = clean(next);
    if (equal(cleaned, patterns)) return;
    setPending(cleaned);
    setError(null);
    const outcome = await onCommit(cleaned);
    setPending(null);
    if (!outcome.ok) setError(outcome.error.message);
  };

  const set = (next: string[]) => (onCommit ? setDraft(next) : onChange?.(next));

  const addRow = (at = rows.length) => {
    set([...rows.slice(0, at), "", ...rows.slice(at)]);
    focusRow.current = at;
  };

  const removeRow = (index: number) => {
    const next = rows.filter((_, i) => i !== index);
    if (onCommit) void save(next);
    else onChange?.(next);
  };

  const onRowKeyDown = (event: KeyboardEvent<HTMLInputElement>, index: number) => {
    if (event.key === "Enter") {
      event.preventDefault();
      addRow(index + 1);
    } else if (event.key === "Backspace" && rows[index] === "" && rows.length > 0) {
      event.preventDefault();
      set(rows.filter((_, i) => i !== index));
      focusRow.current = Math.max(0, index - 1);
    } else if (event.key === "Escape" && onCommit && draft !== null) {
      // Put the saved rows back instead of closing the sheet.
      event.stopPropagation();
      setDraft(null);
    }
  };

  const onBlur = (event: FocusEvent<HTMLDivElement>) => {
    if (onCommit && draft !== null && !event.currentTarget.contains(event.relatedTarget)) void save(draft);
  };

  return (
    <div ref={listRef} role="group" aria-label={label} onBlur={onBlur} className="flex flex-col gap-1.5">
      {rows.length === 0 && <p className="text-[12.5px] text-muted-foreground">None yet.</p>}
      {rows.map((row, index) => (
        <div key={index} className="flex items-center gap-1">
          <Input
            data-pattern-row
            value={row}
            placeholder={placeholder}
            aria-label={`${label}, row ${index + 1}`}
            onChange={(event) => set(rows.map((value, i) => (i === index ? event.target.value : value)))}
            onKeyDown={(event) => onRowKeyDown(event, index)}
            autoComplete="off"
            spellCheck={false}
            className="h-8 flex-1 rounded-md font-mono text-[12.5px] md:text-[12.5px]"
          />
          <PressButton
            variant="ghost"
            size="icon-sm"
            pressScale={0.9}
            aria-label={`Remove ${row || "empty row"}`}
            onClick={() => removeRow(index)}
            className="size-8 rounded-md text-muted-foreground hover:text-foreground"
          >
            <X className="size-3.5" />
          </PressButton>
        </div>
      ))}
      <PressButton variant="ghost" size="sm" onClick={() => addRow()} className="h-7 w-fit rounded-md px-2 text-[12.5px] text-muted-foreground">
        <Plus data-icon="inline-start" />
        Add pattern
      </PressButton>
      {error && (
        <p role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
