import { X } from "lucide-react";
import { useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";

interface TagEditorProps {
  tags: readonly string[];
  /** Tags used elsewhere in the library, offered while typing. */
  suggestions: readonly string[];
  onChange: (tags: string[]) => void;
  disabled?: boolean;
  /** For the input's `aria-labelledby` / `aria-describedby`. */
  labelId?: string;
  describedBy?: string;
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/**
 * Free-text tags as chips. Enter or comma adds what's typed (or the highlighted suggestion),
 * Backspace in an empty input removes the last tag, arrows move through the suggestions.
 */
export function TagEditor({ tags, suggestions, onChange, disabled, labelId, describedBy }: TagEditorProps) {
  const [text, setText] = useState("");
  const [focused, setFocused] = useState(false);
  const [highlight, setHighlight] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();

  const options = useMemo(() => {
    const needle = text.trim().toLowerCase();
    return suggestions
      .filter((tag) => !tags.some((existing) => same(existing, tag)))
      .filter((tag) => tag.toLowerCase().includes(needle))
      .slice(0, 8);
  }, [suggestions, tags, text]);

  const open = focused && options.length > 0;
  const active = open && highlight >= 0 && highlight < options.length ? highlight : -1;

  const addTag = (raw: string) => {
    const tag = raw.trim();
    setText("");
    setHighlight(-1);
    if (!tag || tags.some((existing) => same(existing, tag))) return;
    onChange([...tags, tag]);
  };

  const removeTag = (index: number) => {
    onChange(tags.filter((_, i) => i !== index));
    inputRef.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      addTag(active >= 0 ? options[active] : text);
    } else if (event.key === "Backspace" && text === "" && tags.length > 0) {
      event.preventDefault();
      removeTag(tags.length - 1);
    } else if (event.key === "ArrowDown" && open) {
      event.preventDefault();
      setHighlight((active + 1) % options.length);
    } else if (event.key === "ArrowUp" && open) {
      event.preventDefault();
      setHighlight(active <= 0 ? options.length - 1 : active - 1);
    } else if (event.key === "Escape" && (text || active >= 0)) {
      // First Escape clears the typing; the next one closes the sheet.
      event.stopPropagation();
      setText("");
      setHighlight(-1);
    }
  };

  return (
    <div className="relative">
      <div
        onPointerDown={(event) => {
          // Clicking the box (not a chip's button) puts the caret in the input.
          if (event.target === event.currentTarget) {
            event.preventDefault();
            inputRef.current?.focus();
          }
        }}
        className={cn(
          "flex min-h-9 flex-wrap items-center gap-1 rounded-lg border border-transparent bg-input/50 px-1.5 py-1 transition-[box-shadow,border-color]",
          "has-focus-visible:border-ring has-focus-visible:ring-3 has-focus-visible:ring-ring/30",
          disabled && "opacity-60",
        )}
      >
        {tags.map((tag, index) => (
          <span key={tag} className="flex h-6 items-center gap-0.5 rounded-md bg-background pr-0.5 pl-2 text-[12px] font-medium ring-1 ring-foreground/10">
            {tag}
            <PressButton
              variant="ghost"
              size="icon-xs"
              pressScale={0.9}
              aria-label={`Remove tag ${tag}`}
              disabled={disabled}
              onClick={() => removeTag(index)}
              className="size-5 rounded-sm text-muted-foreground hover:text-foreground"
            >
              <X className="size-3" />
            </PressButton>
          </span>
        ))}
        <Input
          ref={inputRef}
          value={text}
          disabled={disabled}
          onChange={(event) => {
            setText(event.target.value.replace(/,/g, ""));
            setHighlight(-1);
          }}
          onKeyDown={onKeyDown}
          onFocus={() => setFocused(true)}
          onBlur={() => {
            setFocused(false);
            // Leaving with text typed adds it, like pressing Enter.
            if (text.trim()) addTag(text);
          }}
          placeholder={tags.length === 0 ? "Add a family, e.g. web" : "Add another"}
          autoComplete="off"
          spellCheck={false}
          role="combobox"
          aria-labelledby={labelId}
          aria-describedby={describedBy}
          aria-expanded={open}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={active >= 0 ? `${listId}-${active}` : undefined}
          className="h-6 min-w-28 flex-1 rounded-none border-0 bg-transparent px-1 text-[13px] shadow-none focus-visible:border-0 focus-visible:ring-0 md:text-[13px]"
        />
      </div>

      {open && (
        <ul
          id={listId}
          role="listbox"
          aria-label="Tags used in the library"
          className="absolute inset-x-0 top-full z-10 mt-1 flex max-h-48 flex-col overflow-y-auto rounded-xl bg-popover p-1.5 text-popover-foreground shadow-lg ring-1 ring-foreground/10"
        >
          <li role="presentation" className="px-2 pt-0.5 pb-1 text-[11px] text-muted-foreground">
            Used in the library
          </li>
          {options.map((tag, index) => (
            <li
              key={tag}
              id={`${listId}-${index}`}
              role="option"
              aria-selected={index === active}
              data-active={index === active || undefined}
              // Keep focus in the input.
              onPointerDown={(event) => event.preventDefault()}
              onPointerMove={() => setHighlight(index)}
              onClick={() => addTag(tag)}
              className="flex h-7 cursor-default items-center rounded-md px-2 text-[13px] select-none data-active:bg-accent"
            >
              {tag}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
