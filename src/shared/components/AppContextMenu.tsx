import { Fragment, useRef, useState, type MouseEvent, type ReactNode } from "react";
import type { ContextMenu as ContextMenuPrimitive } from "@base-ui/react/context-menu";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { findEditable } from "../lib/context-menu/editing";
import { resolveContextMenu } from "../lib/context-menu/registry";
import { isSubmenu, type ContextMenuEntry, type ContextMenuGroup } from "../lib/context-menu/types";

interface OpenMenu {
  groups: ContextMenuGroup[];
  /** Gets focus back when the menu closes, so actions apply to it. */
  editable: HTMLElement | null;
}

function Entries({ entries, onPick }: { entries: readonly ContextMenuEntry[]; onPick: (run: () => void) => void }) {
  return entries.map((entry) => {
    const Icon = entry.icon;
    if (isSubmenu(entry)) {
      return (
        <ContextMenuSub key={entry.label}>
          <ContextMenuSubTrigger className="gap-2.5">
            {Icon && <Icon className="text-muted-foreground" />}
            {entry.label}
          </ContextMenuSubTrigger>
          <ContextMenuSubContent>
            <Entries entries={entry.items} onPick={onPick} />
          </ContextMenuSubContent>
        </ContextMenuSub>
      );
    }
    return (
      <ContextMenuItem
        key={entry.label}
        disabled={entry.disabled}
        variant={entry.destructive ? "destructive" : "default"}
        onClick={() => onPick(entry.onSelect)}
      >
        {Icon && <Icon className="text-muted-foreground" />}
        {entry.label}
        {entry.shortcut && <ContextMenuShortcut className="pl-6 tracking-normal">{entry.shortcut}</ContextMenuShortcut>}
      </ContextMenuItem>
    );
  });
}

/**
 * Replaces the webview's native right-click menu everywhere with the app's own. What it shows
 * depends on what was right-clicked: groups from regions registered with `useContextMenu`,
 * plus cut/copy/paste for text fields and copy for selected text. Nothing to offer → no menu.
 * In development, Shift+right-click still opens the native menu (for Inspect).
 */
export function AppContextMenu({ children }: { children: ReactNode }) {
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const latest = useRef<OpenMenu | null>(null);
  const pending = useRef<(() => void) | null>(null);
  const actions = useRef<ContextMenuPrimitive.Root.Actions>(null);

  // Capture phase, so it runs on every right-click: Base UI doesn't report a second one while open.
  const onContextMenuCapture = (event: MouseEvent) => {
    if (import.meta.env.DEV && event.shiftKey) {
      event.stopPropagation();
      return;
    }
    const target = event.target instanceof Element ? event.target : null;
    const groups = target ? resolveContextMenu(target) : [];
    const next = groups.length > 0 && target ? { groups, editable: findEditable(target) } : null;
    latest.current = next;
    if (next) setMenu(next);
    else actions.current?.close();
  };

  return (
    <ContextMenu
      actionsRef={actions}
      onOpenChange={(open, details) => {
        if (open && !latest.current) details.cancel();
      }}
      onOpenChangeComplete={(open) => {
        if (open) return;
        const run = pending.current;
        pending.current = null;
        run?.();
      }}
    >
      {/* `contents`: no box of its own; `select-text` undoes the trigger's select-none. */}
      <ContextMenuTrigger className="contents select-text" onContextMenuCapture={onContextMenuCapture}>
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent
        className="min-w-52"
        finalFocus={() => menu?.editable ?? true}
        onContextMenu={(event) => event.preventDefault()}
      >
        {menu?.groups.map((group, index) => (
          <Fragment key={index}>
            {index > 0 && <ContextMenuSeparator />}
            <Entries entries={group} onPick={(run) => (pending.current = run)} />
          </Fragment>
        ))}
      </ContextMenuContent>
    </ContextMenu>
  );
}
