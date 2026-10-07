import { FileText, SquareTerminal } from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import type { AgentEvent } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { EventStream } from "../hooks/useAgentDetail";
import { useArrivals } from "../hooks/useArrivals";
import { fullTime, isAlert, shortAge } from "../lib/events";
import { KIND_ICONS } from "./eventIcons";
import { LoadOlder, SectionPill, StreamState } from "./TabStates";

interface ActivityTabProps {
  stream: EventStream;
  /** Re-renders the relative times. */
  now: number;
  onLoadOlder: () => void;
  onRetry: () => void;
}

/** Everything that isn't a finished turn, a warning or a danger, newest first. */
export function ActivityTab({ stream, now, onLoadOlder, onRetry }: ActivityTabProps) {
  const items = stream.events.filter((event) => event.kind !== "turn_complete" && !isAlert(event));
  const arrived = useArrivals(
    items.map((item) => item.id),
    stream.status === "ready",
  );

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-y-auto overscroll-contain px-4 pt-3 pb-4">
      <SectionPill>Changes</SectionPill>
      <StreamState stream={stream} empty={items.length === 0} onRetry={onRetry} onLoadOlder={onLoadOlder} />
      {stream.status === "ready" && items.length > 0 && (
        <>
          <ul aria-label="Changes" className="-mx-1 flex flex-col">
            {items.map((item) => (
              <ActivityRow key={item.id} event={item} now={now} animate={arrived(item.id)} />
            ))}
          </ul>
          <LoadOlder stream={stream} onLoad={onLoadOlder} />
        </>
      )}
    </div>
  );
}

function ActivityRow({ event, now, animate }: { event: AgentEvent; now: number; animate: boolean }) {
  const [open, setOpen] = useState(false);
  const expandable = Boolean(event.path || event.command || event.detail);
  const Icon = KIND_ICONS[event.kind];

  const line = (
    <>
      <Icon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate text-left">{event.title}</span>
      <time
        dateTime={new Date(event.ts).toISOString()}
        title={fullTime(event.ts)}
        className="shrink-0 text-[11px] text-muted-foreground tabular-nums"
      >
        {shortAge(event.ts, now)}
      </time>
    </>
  );

  return (
    <motion.li
      initial={animate ? { opacity: 0, y: -4 } : false}
      animate={{ opacity: 1, y: 0 }}
      transition={{ type: "spring", visualDuration: 0.25, bounce: 0 }}
    >
      {expandable ? (
        <PressButton
          type="button"
          variant="ghost"
          pressScale={0.98}
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
          className="h-8 w-full justify-start gap-2 rounded-lg px-1.5 text-[12px] font-normal aria-expanded:bg-transparent hover:aria-expanded:bg-muted"
        >
          {line}
        </PressButton>
      ) : (
        <div className="flex h-8 items-center gap-2 px-1.5 text-[12px]">{line}</div>
      )}
      {open && (
        <motion.div
          initial={{ opacity: 0, y: -2 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.15 }}
          className="mx-1.5 mb-1.5 flex flex-col gap-1.5 rounded-lg bg-pill px-2.5 py-2"
        >
          {event.detail && <p className="text-[11.5px] leading-snug whitespace-pre-wrap text-muted-foreground">{event.detail}</p>}
          {event.path && <Code icon={FileText} text={event.path} />}
          {event.command && <Code icon={SquareTerminal} text={event.command} />}
        </motion.div>
      )}
    </motion.li>
  );
}

function Code({ icon: Icon, text }: { icon: typeof FileText; text: string }) {
  return (
    <span className="flex min-w-0 items-start gap-1.5">
      <Icon aria-hidden className="mt-px size-3 shrink-0 text-muted-foreground" />
      <code className="min-w-0 font-mono text-[11px] break-all whitespace-pre-wrap select-text">{text}</code>
    </span>
  );
}
