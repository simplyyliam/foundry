import { ClipboardPaste, Copy, Link, Redo2, Scissors, TextCursorInput, TextSelect, Undo2 } from "lucide-react";
import { shortcut } from "../../utils/keys";
import type { ContextMenuGroup, ContextMenuRequest } from "./types";

/**
 * The menu every text field gets without registering anything: undo/redo, cut/copy/paste and
 * select all in editable fields, copy for any selected text, and copy link address on links.
 *
 * Commands go through the field itself. Text inputs use `execCommand` (it keeps their native
 * undo). A contenteditable gets the same synthetic key or paste event a keyboard user would
 * produce, so an editor that handles its own (ProseMirror/TipTap) runs its own logic; when
 * nothing handles it, the browser default runs instead.
 */

const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

type TextField = HTMLInputElement | HTMLTextAreaElement;

const TEXT_INPUT = "input:not([type]), input[type=text], input[type=search], input[type=url], input[type=email], input[type=tel], input[type=password], input[type=number], textarea";

/** The text field or contenteditable `target` is in. */
export function findEditable(target: Element): HTMLElement | null {
  const field = target.closest<HTMLElement>(TEXT_INPUT);
  if (field) return field;
  const editable = target.closest<HTMLElement>("[contenteditable]");
  // A read-only editor (contenteditable="false") still offers copy.
  return editable;
}

const isTextField = (element: HTMLElement): element is TextField =>
  element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement;

function isWritable(element: HTMLElement) {
  return isTextField(element) ? !element.readOnly && !element.disabled : element.isContentEditable;
}

/** The selected text in `editable`, or in the page when there's no field. */
export function selectedText(editable: HTMLElement | null): string {
  if (editable && isTextField(editable)) {
    try {
      const { selectionStart, selectionEnd, value } = editable;
      return selectionStart === null || selectionEnd === null ? "" : value.slice(selectionStart, selectionEnd);
    } catch {
      return ""; // Inputs like type=number have no selection API.
    }
  }
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed) return "";
  if (editable && !editable.contains(selection.anchorNode)) return "";
  return selection.toString();
}

const focus = (element: HTMLElement) => element.focus({ preventScroll: true });

/** A synthetic shortcut on `element`; true when something handled it. */
function pressShortcut(element: HTMLElement, key: string, shift = false): boolean {
  const event = new KeyboardEvent("keydown", {
    key,
    code: `Key${key.toUpperCase()}`,
    ctrlKey: !isMac,
    metaKey: isMac,
    shiftKey: shift,
    bubbles: true,
    cancelable: true,
  });
  return !element.dispatchEvent(event);
}

function undo(element: HTMLElement, redo: boolean) {
  focus(element);
  if (isTextField(element) || !pressShortcut(element, "z", redo)) document.execCommand(redo ? "redo" : "undo");
}

async function paste(element: HTMLElement, plain: boolean) {
  let text = "";
  let html: string | null = null;
  try {
    if (!plain && navigator.clipboard.read) {
      for (const item of await navigator.clipboard.read()) {
        if (item.types.includes("text/html")) html = await (await item.getType("text/html")).text();
        if (item.types.includes("text/plain")) text = await (await item.getType("text/plain")).text();
      }
    } else {
      text = await navigator.clipboard.readText();
    }
  } catch (error) {
    // The webview refused clipboard access; Ctrl+V still works.
    console.warn("[context menu] Couldn't read the clipboard:", error);
    return;
  }

  focus(element);
  if (!isTextField(element)) {
    const data = new DataTransfer();
    data.setData("text/plain", text);
    if (html !== null) data.setData("text/html", html);
    const event = new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true });
    if (!element.dispatchEvent(event)) return; // The editor pasted it.
  }
  document.execCommand("insertText", false, text);
}

function selectAll(element: HTMLElement) {
  focus(element);
  if (isTextField(element)) element.select();
  else if (!pressShortcut(element, "a")) document.execCommand("selectAll");
}

export function editingGroups({ editable, link, selection }: ContextMenuRequest): ContextMenuGroup[] {
  const groups: ContextMenuGroup[] = [];

  if (editable) {
    const writable = isWritable(editable);
    const hasSelection = selection !== "";
    if (writable) {
      groups.push([
        { label: "Undo", icon: Undo2, shortcut: shortcut("Z"), onSelect: () => undo(editable, false) },
        { label: "Redo", icon: Redo2, shortcut: shortcut("Z", { shift: true }), onSelect: () => undo(editable, true) },
      ]);
    }
    groups.push([
      ...(writable
        ? [{ label: "Cut", icon: Scissors, shortcut: shortcut("X"), disabled: !hasSelection, onSelect: () => (focus(editable), document.execCommand("cut")) }]
        : []),
      { label: "Copy", icon: Copy, shortcut: shortcut("C"), disabled: !hasSelection, onSelect: () => (focus(editable), document.execCommand("copy")) },
      ...(writable
        ? [
            { label: "Paste", icon: ClipboardPaste, shortcut: shortcut("V"), onSelect: () => void paste(editable, false) },
            ...(isTextField(editable)
              ? []
              : [{ label: "Paste as plain text", icon: TextCursorInput, shortcut: shortcut("V", { shift: true }), onSelect: () => void paste(editable, true) }]),
          ]
        : []),
    ]);
    groups.push([{ label: "Select all", icon: TextSelect, shortcut: shortcut("A"), onSelect: () => selectAll(editable) }]);
  } else if (selection !== "") {
    groups.push([{ label: "Copy", icon: Copy, shortcut: shortcut("C"), onSelect: () => document.execCommand("copy") }]);
  }

  if (link) {
    const href = link.href;
    groups.push([{ label: "Copy link address", icon: Link, onSelect: () => void navigator.clipboard.writeText(href) }]);
  }
  return groups;
}
