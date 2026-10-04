import { editingGroups, findEditable, selectedText } from "./editing";
import type { ContextMenuGroup, ContextMenuProvider, ContextMenuRequest } from "./types";

/** Providers registered on DOM elements by `useContextMenu`. */
const providers = new WeakMap<Element, Set<{ current: ContextMenuProvider }>>();

export function registerContextMenu(element: Element, provider: { current: ContextMenuProvider }): () => void {
  const set = providers.get(element) ?? new Set();
  set.add(provider);
  providers.set(element, set);
  return () => {
    set.delete(provider);
    if (set.size === 0) providers.delete(element);
  };
}

/**
 * The menu for a right-click on `target`: the groups of every registered region it's in,
 * innermost first, then the built-in editing groups. Empty means no menu at all.
 */
export function resolveContextMenu(target: Element): ContextMenuGroup[] {
  const editable = findEditable(target);
  const request: ContextMenuRequest = {
    target,
    editable,
    link: target.closest("a[href]"),
    selection: selectedText(editable),
  };

  const groups: ContextMenuGroup[] = [];
  for (let element: Element | null = target; element; element = element.parentElement) {
    for (const provider of providers.get(element) ?? []) groups.push(...(provider.current(request) ?? []));
  }
  groups.push(...editingGroups(request));
  return groups.filter((group) => group.length > 0);
}
