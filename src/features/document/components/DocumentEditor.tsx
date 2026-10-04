import { useRef } from "react";
import type { Editor } from "@tiptap/core";
import { EditorContent } from "@tiptap/react";
import { useDocumentContextMenu } from "../hooks/useDocumentContextMenu";
import { SelectionMenu } from "./SelectionMenu";
import "./document.css";

interface DocumentEditorProps {
  editor: Editor;
  /** The element that scrolls the document, so the selection menu follows it. */
  scrollTarget?: HTMLElement | null;
}

/**
 * The writing surface: markdown shortcuts while typing, a small menu over a selection, and
 * formatting in the right-click menu.
 */
export function DocumentEditor({ editor, scrollTarget }: DocumentEditorProps) {
  const ref = useRef<HTMLDivElement>(null);
  useDocumentContextMenu(ref, editor);
  return (
    <div
      ref={ref}
      className="mx-auto w-full max-w-[680px] px-10 pt-4 pb-[30vh] animate-in fade-in duration-300 motion-reduce:animate-none">
      <EditorContent editor={editor} />
      {/* Mounted once the scroll area exists: TipTap drops the first options update after the
          menu registers, so a scroll area that arrives late would never become its boundary. */}
      {scrollTarget && <SelectionMenu editor={editor} scrollTarget={scrollTarget} />}
    </div>
  );
}
