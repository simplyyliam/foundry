import { motion } from "motion/react";
import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { Message, MessageAvatar, MessageContent } from "@/components/ui/message";
import type { AgentEvent } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import type { EventStream } from "../hooks/useAgentDetail";
import { useArrivals } from "../hooks/useArrivals";
import { clockTime, fullTime, isAlert } from "../lib/events";
import { AgentAvatar } from "./AgentAvatar";
import { LoadOlder, StreamState } from "./TabStates";

interface ImplementationsTabProps {
  stream: EventStream;
  avatar: string;
  onLoadOlder: () => void;
  onRetry: () => void;
}

/**
 * The agent's finished turns as chat bubbles, oldest at the top. The list is a
 * `column-reverse` scroller, so it starts at the bottom and stays anchored there as turns
 * arrive, while older ones loaded above don't move what's on screen.
 */
export function ImplementationsTab({ stream, avatar, onLoadOlder, onRetry }: ImplementationsTabProps) {
  const turns = stream.events.filter((event) => event.kind === "turn_complete" && !isAlert(event));
  const arrived = useArrivals(
    turns.map((turn) => turn.id),
    stream.status === "ready",
  );
  const state = <StreamState stream={stream} empty={turns.length === 0} onRetry={onRetry} onLoadOlder={onLoadOlder} />;
  if (stream.status !== "ready" || turns.length === 0) return <div className="px-4 pt-2">{state}</div>;

  return (
    <div className="flex h-full min-h-0 flex-col-reverse gap-2.5 overflow-y-auto overscroll-contain px-4 pt-2 pb-4">
      {/* Newest first in the DOM; column-reverse puts it at the bottom. */}
      {turns.map((turn) => (
        <Turn key={turn.id} turn={turn} avatar={avatar} animate={arrived(turn.id)} />
      ))}
      <LoadOlder stream={stream} onLoad={onLoadOlder} />
    </div>
  );
}

function Turn({ turn, avatar, animate }: { turn: AgentEvent; avatar: string; animate: boolean }) {
  // Only the event's own text: a turn without a last message gets a muted placeholder.
  const text = turn.detail?.trim() ? turn.detail : null;
  return (
    <motion.div
      initial={animate ? { opacity: 0, y: 6 } : false}
      animate={{ opacity: 1, y: 0 }}
      transition={{ type: "spring", visualDuration: 0.3, bounce: 0.15 }}
    >
      <Message className="items-end gap-2">
        <MessageAvatar className="size-5 min-w-0 bg-transparent">
          <AgentAvatar src={avatar} className="size-5" />
        </MessageAvatar>
        <MessageContent>
          <Bubble variant="muted" className="max-w-full">
            <BubbleContent className="flex flex-col gap-1 rounded-[12px] bg-pill! px-2.5 py-2 text-[12px] leading-snug">
              <p className={cn("whitespace-pre-wrap", !text && "text-muted-foreground")}>{text ?? "Finished a turn"}</p>
              <time
                dateTime={new Date(turn.ts).toISOString()}
                title={fullTime(turn.ts)}
                className="self-end text-[9.5px] text-muted-foreground tabular-nums"
              >
                {clockTime(turn.ts)}
              </time>
            </BubbleContent>
          </Bubble>
        </MessageContent>
      </Message>
    </motion.div>
  );
}
