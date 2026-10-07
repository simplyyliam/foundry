import { ClipboardPaste, Copy } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import { cn } from "@/lib/utils";
import { shortcut, useContextMenu } from "@/shared";
import type { TerminalRegistry } from "../hooks/terminalRegistry";

interface TerminalSlotProps {
  registry: TerminalRegistry | null;
  agentId: string;
  /** Holds the agent's terminal while true. Only one slot per agent is active at a time. */
  active: boolean;
  /** Whether the session takes input (Paste is offered only then). */
  live: boolean;
  className?: string;
}

/**
 * Where an agent's terminal shows: its card, or the expanded dialog. The terminal itself lives
 * in the registry, so moving it between slots keeps its scrollback and selection. Every size
 * change of the slot refits the terminal and (debounced) resizes the session.
 */
export function TerminalSlot({ registry, agentId, active, live, className }: TerminalSlotProps) {
  const ref = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element || !registry || !active) return;
    registry.attach(agentId, element);
    const observer = new ResizeObserver(() => registry.scheduleResize(agentId));
    observer.observe(element);
    return () => {
      observer.disconnect();
      registry.detach(agentId, element);
    };
  }, [registry, agentId, active]);

  // xterm's selection isn't a DOM selection, so the app menu's own Copy can't see it.
  useContextMenu(ref, () => {
    if (!registry) return null;
    const selection = registry.selectionOf(agentId);
    return [
      [
        {
          label: "Copy",
          icon: Copy,
          shortcut: shortcut("C"),
          disabled: !selection,
          onSelect: () => void navigator.clipboard.writeText(selection).catch((error) => console.warn("Copy failed:", error)),
        },
        {
          label: "Paste",
          icon: ClipboardPaste,
          shortcut: shortcut("V"),
          disabled: !live,
          onSelect: () =>
            void navigator.clipboard
              .readText()
              .then((text) => registry.paste(agentId, text))
              .catch((error) => console.warn("Paste failed:", error)),
        },
      ],
    ];
  });

  return <div ref={ref} className={cn("relative", className)} />;
}
