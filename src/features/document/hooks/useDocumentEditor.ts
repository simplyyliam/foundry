import { useEffect, useEffectEvent, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Editor } from "@tiptap/core";
import { useEditor } from "@tiptap/react";
import { documentExtensions } from "../lib/extensions";
import { createSourceMap, type SourceMap } from "../lib/markdown-source";
import { setUncoveredBlocks, uncoveredBlocks } from "../lib/uncovered-highlight";
import { Autosave, IDLE, type SaveStatus } from "./autosave";
import type { LoadedDocument } from "./documentFile";

const PLACEHOLDER = "Dump everything about the idea here: who it's for, what it does, what it must never do…";

interface DocumentEditorOptions {
  projectId: string;
  /** What was loaded; null when loading failed (the editor is then empty and read-only). */
  loaded: LoadedDocument | null;
  /** `getRoadmap`'s uncovered paragraphs, or empty. */
  uncovered: readonly string[];
}

export interface DocumentEditorState {
  editor: Editor;
  status: SaveStatus;
}

/**
 * The editor for one project's document, with autosave: ~800 ms after the last edit, and
 * immediately on blur, unmount (leaving the page or switching project) and window close.
 * Mount it once per project (keyed by project id); it reads `loaded` only at mount.
 */
export function useDocumentEditor({ projectId, loaded, uncovered }: DocumentEditorOptions): DocumentEditorState {
  const [status, setStatus] = useState<SaveStatus>(IDLE);
  const autosave = useRef<Autosave | null>(null);
  const map = useRef<SourceMap | null>(null);
  const readOnly = loaded === null;

  const editor = useEditor(
    {
      extensions: documentExtensions(PLACEHOLDER),
      content: loaded ? (loaded.draft ?? loaded.file) : "",
      contentType: "markdown",
      editable: !readOnly,
      immediatelyRender: true,
      // The editor re-renders nothing on keystrokes; the selection menu subscribes on its own.
      shouldRerenderOnTransaction: false,
      editorProps: {
        attributes: { class: "doc-prose", role: "textbox", "aria-multiline": "true", "aria-label": "Idea" },
      },
      onUpdate: ({ editor }) => autosave.current?.edited(editor.state.doc),
      onBlur: () => void autosave.current?.flush(),
    },
    [],
  );

  // Re-run whenever the uncovered list or the saved text (the source map) changes.
  const highlight = useEffectEvent(() => {
    if (map.current) setUncoveredBlocks(editor, uncoveredBlocks(map.current, uncovered));
  });

  // Autosave lives from mount to unmount; unmounting saves the pending edit.
  useEffect(() => {
    if (!loaded || !editor.markdown) return;
    map.current = createSourceMap(loaded.draft ?? loaded.file, editor.state.doc, editor.markdown, editor.schema);
    const session = new Autosave({
      projectId,
      manager: editor.markdown,
      map: map.current,
      file: loaded.file,
      onStatus: (next) => setStatus(next),
      onSaved: (saved) => {
        map.current = saved;
        highlight();
      },
    });
    autosave.current = session;
    highlight();
    // Text from an earlier failed save: write it now.
    if (loaded.draft !== null) {
      session.edited(editor.state.doc);
      void session.flush();
    }

    const onUnload = () => void session.flush();
    window.addEventListener("beforeunload", onUnload);
    return () => {
      window.removeEventListener("beforeunload", onUnload);
      void session.flush();
      if (autosave.current === session) autosave.current = null;
    };
  }, [projectId, loaded, editor]);

  useEffect(() => highlight(), [uncovered]);

  useSaveOnClose(autosave, !readOnly);

  return { editor, status };
}

/**
 * Saves before the window closes. The capabilities don't allow `destroy()`, so instead of
 * letting Tauri's helper destroy the window, this keeps it open, saves, stops listening and
 * closes it again (`close()` is allowed, and with no listener left it just closes). If the
 * save fails the window stays open once, showing the error; closing again closes anyway.
 */
function useSaveOnClose(autosave: { current: Autosave | null }, enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    let appWindow: ReturnType<typeof getCurrentWindow>;
    try {
      appWindow = getCurrentWindow();
    } catch {
      return; // Not running inside Tauri.
    }

    let unlisten: (() => unknown) | undefined;
    let disposed = false;
    let keptOpenOnce = false;
    appWindow
      .onCloseRequested(async (event) => {
        event.preventDefault();
        const saved = (await autosave.current?.flush()) ?? true;
        if (!saved && !keptOpenOnce) {
          keptOpenOnce = true;
          return;
        }
        const stop = unlisten;
        unlisten = undefined;
        disposed = true;
        await stop?.();
        await appWindow.close();
      })
      .then((stop) => {
        if (disposed) void stop();
        else unlisten = stop;
      })
      .catch(() => {});

    return () => {
      disposed = true;
      void unlisten?.();
      unlisten = undefined;
    };
  }, [autosave, enabled]);
}
