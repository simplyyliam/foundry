import { Check, Pencil } from "lucide-react";
import { useId, useState } from "react";
import { Badge } from "@/components/ui/badge";
import type { Placeholder } from "@/lib/bindings";
import { PressButton, TagEditor, useSettingsDialog } from "@/shared";
import type { Outcome } from "../hooks/call";

interface DesignFamilyBlockProps {
  /** The `designFamily` placeholder (`inTemplate: false`). */
  placeholder: Placeholder;
  /** The tags being picked, or null when a confirmed value isn't being edited. */
  draft: string[] | null;
  onDraftChange: (tags: string[] | null) => void;
  onConfirm: (tags: string[]) => Promise<Outcome<unknown>>;
  disabled: boolean;
}

/** Comma-separated, as the backend stores it ("web, mobile"). */
const parseFamilies = (value: string | null) =>
  (value ?? "")
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean);

/**
 * The project's design family: which Design Library files get copied in. Not part of the
 * file's text. Tags with suggestions from the library, free text allowed, confirmed like a
 * placeholder (Enter in an empty tag input confirms, Escape cancels an edit).
 */
export function DesignFamilyBlock({ placeholder, draft, onDraftChange, onConfirm, disabled }: DesignFamilyBlockProps) {
  const id = useId();
  const { openSettings } = useSettingsDialog();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editing = !placeholder.confirmed || draft !== null;
  const tags = draft ?? parseFamilies(placeholder.value);
  const canConfirm = !disabled && !busy && (placeholder.optional || tags.length > 0);

  const confirm = async () => {
    if (!canConfirm) return;
    setBusy(true);
    setError(null);
    const outcome = await onConfirm(tags);
    setBusy(false);
    // folder_missing shows once, in the section's banner.
    if (!outcome.ok && outcome.error.code !== "folder_missing") setError(outcome.error.message);
  };

  const cancel = () => {
    setError(null);
    onDraftChange(null);
  };

  return (
    <div
      className="flex flex-col gap-2 rounded-xl bg-muted/50 p-4 ring-1 ring-foreground/5"
      onKeyDown={(event) => {
        // The tag input keeps Escape while there's typing to clear; past that it cancels.
        if (event.key === "Escape" && placeholder.confirmed && draft !== null) {
          event.preventDefault();
          cancel();
        }
      }}
    >
      <div className="flex min-h-7 items-center gap-2">
        <span id={`${id}-label`} className="text-[13px] font-medium">
          Which design guidelines apply?
        </span>
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
              aria-label="Edit design family"
              onClick={() => onDraftChange(parseFamilies(placeholder.value))}
              className="rounded-md text-muted-foreground"
            >
              <Pencil data-icon="inline-start" />
              Edit
            </PressButton>
          </>
        )}
      </div>
      <p id={`${id}-note`} className="-mt-1 text-[12px] leading-snug text-muted-foreground">
        Only Design Library files tagged with a family you pick here are copied into this project.
      </p>

      {editing ? (
        <>
          <TagEditor
            tags={tags}
            suggestions={placeholder.suggestions}
            onChange={(next) => onDraftChange(next)}
            onSubmit={() => void confirm()}
            disabled={disabled}
            labelId={`${id}-label`}
            describedBy={`${id}-note${error ? ` ${id}-error` : ""}`}
          />
          {placeholder.suggestions.length === 0 && (
            <p className="text-[12px] leading-snug text-muted-foreground">
              No families yet. The tags come from{" "}
              <PressButton
                variant="link"
                onClick={() => openSettings("design-library")}
                className="h-auto p-0 text-[12px] font-medium text-foreground"
              >
                Settings → Design Library
              </PressButton>
              .
            </p>
          )}
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
        <div className="flex flex-wrap gap-1">
          {tags.length === 0 ? (
            <span className="text-[13px] text-muted-foreground italic">No family picked</span>
          ) : (
            tags.map((tag) => (
              <Badge key={tag} variant="secondary" className="h-6 rounded-md bg-background px-2 text-[12px] ring-1 ring-foreground/10">
                {tag}
              </Badge>
            ))
          )}
        </div>
      )}

      {error && (
        <p id={`${id}-error`} role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
