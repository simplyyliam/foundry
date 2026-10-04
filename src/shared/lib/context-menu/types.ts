import type { ComponentType } from "react";

export interface ContextMenuAction {
  label: string;
  /** Shown on the right, e.g. `shortcut("Z")`. Display only: it doesn't bind the key. */
  shortcut?: string;
  icon?: ComponentType<{ className?: string }>;
  disabled?: boolean;
  destructive?: boolean;
  /** Runs after the menu closed; focus is already back on the right-clicked field. */
  onSelect: () => void;
}

export interface ContextMenuSubmenu {
  label: string;
  icon?: ComponentType<{ className?: string }>;
  items: readonly ContextMenuEntry[];
}

export type ContextMenuEntry = ContextMenuAction | ContextMenuSubmenu;

/** Entries shown together; groups are separated by a line. */
export type ContextMenuGroup = readonly ContextMenuEntry[];

/** What was right-clicked. */
export interface ContextMenuRequest {
  /** The element under the pointer. */
  target: Element;
  /** The text field or contenteditable it's in, if any. */
  editable: HTMLElement | null;
  /** The link it's in, if any. */
  link: HTMLAnchorElement | null;
  /** The selected text (in `editable` when there is one), or "". */
  selection: string;
}

/** Builds a region's groups for a right-click inside it. Return nothing to add none. */
export type ContextMenuProvider = (request: ContextMenuRequest) => readonly ContextMenuGroup[] | null | undefined;

export const isSubmenu = (entry: ContextMenuEntry): entry is ContextMenuSubmenu => "items" in entry;
