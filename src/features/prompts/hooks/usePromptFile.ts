import { useCallback, useEffect, useState } from "react";
import { commands } from "@/lib/bindings";
import { call, type Outcome } from "./call";
import { isBlankNote } from "../lib/prompts";

export type PromptText =
  | { status: "loading" }
  | { status: "error"; message: string }
  /** `note` is null without one. */
  | { status: "ready"; text: string; note: string | null };

/** One prompt file's generated text and the user's note (`getPrompt`), read when the sheet shows it (keyed by file). */
export function usePromptText(projectId: string, file: string): PromptText {
  const [text, setText] = useState<PromptText>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    void call(() => commands.getPrompt(projectId, file)).then((outcome) => {
      if (cancelled) return;
      setText(
        outcome.ok
          ? { status: "ready", text: outcome.data.text, note: outcome.data.note }
          : { status: "error", message: outcome.error.message },
      );
    });
    return () => {
      cancelled = true;
    };
  }, [projectId, file]);

  return text;
}

export type NoteLoad = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; note: string | null };

export interface PromptNote {
  load: NoteLoad;
  /** Stores the note (`setPromptNote`); an empty one removes it. */
  save: (note: string) => Promise<Outcome<null>>;
}

/**
 * The user's note on one prompt file. `listPrompts` doesn't carry notes, so each listed prompt
 * reads its own with `getPrompt` (which also returns the text; only the note is kept).
 */
export function usePromptNote(projectId: string, file: string): PromptNote {
  const [load, setLoad] = useState<NoteLoad>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    void call(() => commands.getPrompt(projectId, file)).then((outcome) => {
      if (cancelled) return;
      setLoad(outcome.ok ? { status: "ready", note: outcome.data.note } : { status: "error", message: outcome.error.message });
    });
    return () => {
      cancelled = true;
    };
  }, [projectId, file]);

  const save = useCallback(
    async (note: string) => {
      const outcome = await call(() => commands.setPromptNote(projectId, file, note));
      if (outcome.ok) setLoad({ status: "ready", note: isBlankNote(note) ? null : note });
      return outcome;
    },
    [projectId, file],
  );

  return { load, save };
}

/** The one line that starts an agent's session on a prompt file (`launchMessage`). */
export function useLaunchMessage(projectId: string): (agentId: string, file: string) => Promise<Outcome<string>> {
  return useCallback(
    (agentId: string, file: string) => call(() => commands.launchMessage(projectId, agentId, file)),
    [projectId],
  );
}
