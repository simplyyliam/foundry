import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type { CSSProperties } from "react";
import type { TeamAgent } from "@/lib/bindings";
import type { TerminalRegistry } from "../hooks/terminalRegistry";
import { isLive, type SessionView } from "../lib/session";
import { SessionStatus } from "./SessionStatus";
import { TerminalSlot } from "./TerminalSlot";
import "./agent-tint.css";

interface ExpandedTerminalProps {
  registry: TerminalRegistry | null;
  /** The agent shown; kept while the dialog animates closed. */
  agent: TeamAgent | null;
  avatar: string;
  /** The agent's card colours, so the dialog matches its card. */
  tint: CSSProperties | undefined;
  session: SessionView | undefined;
  open: boolean;
  onClose: () => void;
}

/**
 * One agent's terminal, large, for reading long output that wraps badly at card width. It's
 * the card's own terminal moved here (same instance and scrollback), and moved back on close.
 * Escape inside the terminal goes to Claude (it interrupts), so it doesn't close the dialog.
 */
export function ExpandedTerminal({ registry, agent, avatar, tint, session, open, onClose }: ExpandedTerminalProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={(next, details) => {
        if (next) return;
        const target = details.event?.target;
        if (details.reason === "escape-key" && target instanceof Element && target.closest(".xterm")) {
          details.cancel();
          return;
        }
        onClose();
      }}
    >
      <DialogContent
        initialFocus={() => (agent ? registry?.inputOf(agent.id) : null) ?? true}
        style={tint}
        className="agent-tint flex h-[calc(100%-3rem)] w-[calc(100%-3rem)] max-w-none flex-col gap-3 rounded-[20px] p-4 pt-3.5 sm:max-w-none"
      >
        {agent && (
          <>
            <header className="flex h-7 items-center gap-2 pr-10">
              <img src={avatar} alt="" className="size-5 shrink-0 rounded-full object-cover" />
              <DialogTitle className="flex min-w-0 items-baseline gap-1.5 text-[14px] font-normal">
                <span className="shrink-0 font-medium text-(--tint-name)">{agent.displayName}</span>
                <span className="truncate text-(--tint-title)">{agent.title}</span>
              </DialogTitle>
              <SessionStatus session={session} />
              <DialogDescription className="ml-auto truncate text-[11.5px]">
                Esc goes to Claude. Close with ✕ to put it back in its card.
              </DialogDescription>
            </header>
            <div className="relative min-h-0 flex-1">
              <TerminalSlot
                registry={registry}
                agentId={agent.id}
                active={open}
                live={isLive(session)}
                className="absolute inset-0"
              />
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
