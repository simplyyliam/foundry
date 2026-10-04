import { useEffect, useRef, type RefObject } from "react";
import { registerContextMenu } from "../lib/context-menu/registry";
import type { ContextMenuProvider } from "../lib/context-menu/types";

/**
 * Adds groups to the app's context menu for right-clicks inside `ref`'s element. Called at
 * right-click time with what was clicked; regions nest (innermost groups come first), and
 * text fields get cut/copy/paste on top without registering anything.
 *
 *   useContextMenu(rowRef, () => [[{ label: "Rename", icon: Pencil, onSelect: startRename }]]);
 */
export function useContextMenu(ref: RefObject<Element | null>, provider: ContextMenuProvider): void {
  const latest = useRef(provider);
  useEffect(() => {
    latest.current = provider;
  });

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    return registerContextMenu(element, latest);
  }, [ref]);
}
