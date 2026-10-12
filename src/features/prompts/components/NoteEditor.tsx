import { Check, ChevronRight, LoaderCircle } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import type { PromptNote } from "../hooks/usePromptFile";
import { NOTE_MAX_CHARS, noteLength } from "../lib/prompts";
import { ErrorLine } from "./Notices";

const COUNT = new Intl.NumberFormat("en-US");

type SaveState = { status: "idle" } | { status: "saving" } | { status: "saved" } | { status: "error"; message: string };

/**
 * "Add a note" under a task prompt: a text box that saves on blur, with Save (or Ctrl/⌘+Enter)
 * and Escape to undo unsaved typing. Clearing it and saving removes the note.
 */
export function NoteEditor({ note }: { note: PromptNote }) {
  const id = useId();
  const { load, save } = note;
  const saved = load.status === "ready" ? (load.note ?? "") : "";
  // Null while untouched: the box shows what's saved.
  const [draft, setDraft] = useState<string | null>(null);
  const [state, setState] = useState<SaveState>({ status: "idle" });

  useEffect(() => {
    if (state.status !== "saved") return;
    const timer = setTimeout(() => setState({ status: "idle" }), 1500);
    return () => clearTimeout(timer);
  }, [state.status]);

  const value = draft ?? saved;
  const length = noteLength(value);
  const over = length > NOTE_MAX_CHARS;
  const dirty = draft !== null && draft !== saved;
  const saving = state.status === "saving";

  const commit = async () => {
    if (!dirty || over || saving || load.status !== "ready") return;
    const sending = draft;
    setState({ status: "saving" });
    const outcome = await save(sending);
    if (!outcome.ok) {
      setState({ status: "error", message: outcome.error.message });
      return;
    }
    // Typing that happened while saving stays a draft.
    setDraft((current) => (current === sending ? null : current));
    setState({ status: "saved" });
  };

  return (
    <Collapsible className="group/note flex flex-col gap-1.5 pl-10">
      <CollapsibleTrigger
        render={
          <PressButton
            variant="ghost"
            size="xs"
            className="-ml-2 w-fit gap-1 rounded-md px-2 text-[12px] font-medium text-muted-foreground"
          />
        }
      >
        <ChevronRight
          aria-hidden
          className="size-3 transition-transform duration-150 group-data-[open]/note:rotate-90 motion-reduce:transition-none"
        />
        {load.status === "ready" && load.note !== null ? "Edit note" : "Add a note"}
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-1.5 pb-2">
        {load.status === "error" ? (
          <ErrorLine message={load.message} />
        ) : (
          <>
            <Textarea
              id={id}
              aria-label="Note"
              aria-describedby={`${id}-count ${id}-hint`}
              aria-invalid={over || undefined}
              placeholder={load.status === "loading" ? "Loading…" : "Anything the agent should know that the prompt doesn't say"}
              disabled={load.status !== "ready"}
              value={value}
              onChange={(event) => {
                setDraft(event.target.value);
                if (state.status === "error") setState({ status: "idle" });
              }}
              onBlur={() => void commit()}
              onKeyDown={(event) => {
                if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                  event.preventDefault();
                  void commit();
                } else if (event.key === "Escape" && dirty) {
                  // Undo unsaved typing without closing whatever holds the page.
                  event.stopPropagation();
                  setDraft(null);
                }
              }}
              className="max-h-72 min-h-20 text-[13px] md:text-[13px]"
            />
            <div className="flex items-center gap-2">
              <p id={`${id}-hint`} className="min-w-0 flex-1 text-[12px] leading-snug text-pretty text-muted-foreground">
                Your note is added to the end of the prompt when the agent starts.
              </p>
              {state.status === "saved" && (
                <span role="status" className="flex items-center gap-1 text-[12px] text-muted-foreground">
                  <Check aria-hidden className="size-3" />
                  Saved
                </span>
              )}
              <span
                id={`${id}-count`}
                className={cn("shrink-0 text-[12px] tabular-nums", over ? "font-medium text-destructive" : "text-muted-foreground")}
              >
                {COUNT.format(length)} / {COUNT.format(NOTE_MAX_CHARS)}
              </span>
              <PressButton
                size="xs"
                variant="secondary"
                className="shrink-0 rounded-md"
                disabled={!dirty || over || saving}
                // Keep focus in the box so its blur doesn't save first.
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => void commit()}
              >
                {saving && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
                Save
              </PressButton>
            </div>
            {over && (
              <p role="alert" className="text-[12px] text-destructive">
                {COUNT.format(length - NOTE_MAX_CHARS)} {length - NOTE_MAX_CHARS === 1 ? "character" : "characters"} over the limit.
                Shorten the note to save it.
              </p>
            )}
            {state.status === "error" && <ErrorLine message={state.message} />}
          </>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}
