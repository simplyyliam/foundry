import { X } from "lucide-react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import type { TeamAgent } from "@/lib/bindings";
import { usePressScale } from "@/shared";
import { IconButton } from "@/shared/components/IconButton";
import type { AgentDetail } from "../hooks/useAgentDetail";
import { AgentDetailView } from "./AgentDetailView";

interface DetailPopOutProps {
  detail: AgentDetail;
  agent: TeamAgent;
  avatar: string;
  now: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** The same agent detail (same events and tab) in a large dialog, like the Roadmap's pop-out. */
export function DetailPopOut({ detail, agent, avatar, now, open, onOpenChange }: DetailPopOutProps) {
  const closeRef = usePressScale<HTMLButtonElement>();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(85vh,52rem)] w-[min(44rem,calc(100%-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 pt-1.5 sm:max-w-none"
      >
        <DialogTitle className="sr-only">{agent.displayName}</DialogTitle>
        <AgentDetailView
          detail={detail}
          agent={agent}
          avatar={avatar}
          now={now}
          trailing={
            <DialogClose
              render={
                <IconButton ref={closeRef} Icon={X} size={15} ariaLabel="Close" variant="ghost" className="size-7 rounded-md" />
              }
            />
          }
        />
      </DialogContent>
    </Dialog>
  );
}
