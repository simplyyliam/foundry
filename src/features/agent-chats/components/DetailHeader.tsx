import { ArrowLeft } from "lucide-react";
import type { ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { TeamAgent } from "@/lib/bindings";
import { usePressScale } from "@/shared";
import { IconButton } from "@/shared/components/IconButton";
import { AgentAvatar } from "./AgentAvatar";

interface DetailHeaderProps {
  agent: TeamAgent;
  avatar: string;
  /** Shows the back button (the panel; the pop-out has none). */
  onBack?: () => void;
  /** Right-hand actions: the pop-out button, or the pop-out's close button. */
  trailing?: ReactNode;
}

/** Back, the agent's avatar, name and title, then the pop-out (or close) button. */
export function DetailHeader({ agent, avatar, onBack, trailing }: DetailHeaderProps) {
  const backRef = usePressScale<HTMLButtonElement>();
  return (
    <div className="flex w-full items-center justify-between gap-2 px-2.5">
      <div className="flex min-w-0 items-center gap-2.5">
        {onBack && (
          <Tooltip>
            <TooltipTrigger
              render={
                <IconButton
                  ref={backRef}
                  Icon={ArrowLeft}
                  size={13}
                  ariaLabel="Back to Agent Chats"
                  className="size-6 rounded-md"
                  onClick={onBack}
                />
              }
            />
            <TooltipContent>Back</TooltipContent>
          </Tooltip>
        )}
        <AgentAvatar src={avatar} className="size-6" />
        <div className="flex min-w-0 flex-col">
          <h1 className="truncate text-[14px] leading-tight font-medium">{agent.displayName}</h1>
          <p className="truncate text-[11.5px] leading-tight text-muted-foreground">{agent.title}</p>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">{trailing}</div>
    </div>
  );
}
