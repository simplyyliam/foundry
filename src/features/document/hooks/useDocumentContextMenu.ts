import type { RefObject } from "react";
import type { ChainedCommands, Editor } from "@tiptap/core";
import {
  Bold,
  Code2,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  Link2,
  List,
  ListOrdered,
  Pilcrow,
  Quote,
  Repeat2,
} from "lucide-react";
import { shortcut, useContextMenu, type ContextMenuGroup } from "@/shared";

/**
 * The document's own groups in the app context menu: formatting for a selection, and
 * "Turn into" for the block under the caret. Cut/copy/paste come from the app menu itself.
 */
export function useDocumentContextMenu(ref: RefObject<HTMLElement | null>, editor: Editor) {
  useContextMenu(ref, ({ target }) => {
    if (!editor.isEditable || !editor.view.dom.contains(target)) return null;
    const run = (command: (chain: ChainedCommands) => ChainedCommands) => () => command(editor.chain().focus()).run();
    const is = (name: string, attributes?: Record<string, unknown>) => editor.isActive(name, attributes);
    const groups: ContextMenuGroup[] = [];

    if (!editor.state.selection.empty) {
      groups.push([
        { label: "Bold", icon: Bold, shortcut: shortcut("B"), onSelect: run((c) => c.toggleBold()) },
        { label: "Italic", icon: Italic, shortcut: shortcut("I"), onSelect: run((c) => c.toggleItalic()) },
        {
          label: is("link") ? "Edit link…" : "Link…",
          icon: Link2,
          shortcut: shortcut("K"),
          // Opens the selection menu's link field (it listens for Ctrl/⌘+K).
          onSelect: () => {
            // Synchronously: `commands.focus()` waits a frame and would pull focus back from the field.
            editor.view.focus();
            const mac = /Mac|iPhone|iPad/.test(navigator.platform);
            editor.view.dom.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: !mac, metaKey: mac, bubbles: true }));
          },
        },
      ]);
    }

    const textBlock = is("paragraph") && !is("bulletList") && !is("orderedList") && !is("blockquote");
    groups.push([
      {
        label: "Turn into",
        icon: Repeat2,
        items: [
          { label: "Text", icon: Pilcrow, disabled: textBlock, onSelect: run((c) => c.clearNodes()) },
          { label: "Heading 1", icon: Heading1, disabled: is("heading", { level: 1 }), onSelect: run((c) => c.clearNodes().setHeading({ level: 1 })) },
          { label: "Heading 2", icon: Heading2, disabled: is("heading", { level: 2 }), onSelect: run((c) => c.clearNodes().setHeading({ level: 2 })) },
          { label: "Heading 3", icon: Heading3, disabled: is("heading", { level: 3 }), onSelect: run((c) => c.clearNodes().setHeading({ level: 3 })) },
          { label: "Bulleted list", icon: List, disabled: is("bulletList"), onSelect: run((c) => c.clearNodes().toggleBulletList()) },
          { label: "Numbered list", icon: ListOrdered, disabled: is("orderedList"), onSelect: run((c) => c.clearNodes().toggleOrderedList()) },
          { label: "Quote", icon: Quote, disabled: is("blockquote"), onSelect: run((c) => c.clearNodes().toggleBlockquote()) },
          { label: "Code block", icon: Code2, disabled: is("codeBlock"), onSelect: run((c) => c.clearNodes().toggleCodeBlock()) },
        ],
      },
    ]);
    return groups;
  });
}
