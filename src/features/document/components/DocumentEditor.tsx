import type { Editor } from "@tiptap/core";
import { EditorContent } from "@tiptap/react";
import { SelectionMenu } from "./SelectionMenu";
import "./document.css";

interface DocumentEditorProps {
  editor: Editor;
  /** The element that scrolls the document, so the selection menu follows it. */
  scrollTarget?: HTMLElement | null;
}

/** The writing surface: markdown shortcuts while typing, a small menu over a selection. */
export function DocumentEditor({ editor, scrollTarget }: DocumentEditorProps) {
  return (
    <div className="mx-auto w-full max-w-[680px] px-10 pt-4 pb-[30vh] animate-in fade-in duration-300 motion-reduce:animate-none">
      <EditorContent editor={editor} />
      <SelectionMenu editor={editor} scrollTarget={scrollTarget} />
    </div>
  );
}
