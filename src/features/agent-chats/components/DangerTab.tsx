import { FileText, OctagonAlert, SquareTerminal, TriangleAlert } from "lucide-react";
import { motion } from "motion/react";
import type { AgentEvent } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import type { EventStream } from "../hooks/useAgentDetail";
import { useArrivals } from "../hooks/useArrivals";
import { clockTime, fullTime } from "../lib/events";
import { LoadOlder, SectionPill, StreamState } from "./TabStates";

interface DangerTabProps {
  stream: EventStream;
  onLoadOlder: () => void;
  onRetry: () => void;
}

/** Warnings and dangers (blocked, needs attention, stuck, …), newest first. */
export function DangerTab({ stream, onLoadOlder, onRetry }: DangerTabProps) {
  const arrived = useArrivals(
    stream.events.map((event) => event.id),
    stream.status === "ready",
  );
  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-y-auto overscroll-contain px-4 pt-3 pb-4">
      <SectionPill>Danger</SectionPill>
      <StreamState stream={stream} empty={stream.events.length === 0} onRetry={onRetry} onLoadOlder={onLoadOlder} />
      {stream.status === "ready" && stream.events.length > 0 && (
        <>
          <ul aria-label="Warnings and dangers" className="flex flex-col gap-2">
            {stream.events.map((event) => (
              <DangerCard key={event.id} event={event} animate={arrived(event.id)} />
            ))}
          </ul>
          <LoadOlder stream={stream} onLoad={onLoadOlder} />
        </>
      )}
    </div>
  );
}

const LEVELS = {
  danger: { icon: OctagonAlert, label: "Danger", className: "text-destructive" },
  // A warning often means the agent is stalled until you act ("Needs permission").
  warn: { icon: TriangleAlert, label: "Warning", className: "text-warning" },
} as const;

function DangerCard({ event, animate }: { event: AgentEvent; animate: boolean }) {
  const level = LEVELS[event.severity === "danger" ? "danger" : "warn"];
  const LevelIcon = level.icon;
  return (
    <motion.li
      initial={animate ? { opacity: 0, y: -4 } : false}
      animate={{ opacity: 1, y: 0 }}
      transition={{ type: "spring", visualDuration: 0.3, bounce: 0.1 }}
      className="flex flex-col gap-1 rounded-[14px] bg-pill px-3 pt-2.5 pb-2"
    >
      <span className={cn("flex items-center gap-1 text-[10.5px] font-medium", level.className)}>
        <LevelIcon aria-hidden className="size-3 shrink-0" />
        {level.label}
      </span>
      <p className="text-[12.5px] leading-snug font-medium wrap-break-word">{event.title}</p>
      {event.detail && (
        <p className="text-[11.5px] leading-snug wrap-break-word whitespace-pre-wrap text-muted-foreground">{event.detail}</p>
      )}
      {(event.path || event.command) && (
        <div className="mt-1.5 flex flex-col gap-1">
          {event.path && <Chip icon={FileText} text={event.path} />}
          {event.command && <Chip icon={SquareTerminal} text={event.command} />}
        </div>
      )}
      <time
        dateTime={new Date(event.ts).toISOString()}
        title={fullTime(event.ts)}
        className="self-end text-[9.5px] text-muted-foreground tabular-nums"
      >
        {clockTime(event.ts)}
      </time>
    </motion.li>
  );
}

function Chip({ icon: Icon, text }: { icon: typeof FileText; text: string }) {
  return (
    <span className="flex w-fit max-w-full min-w-0 items-start gap-1.5 rounded-lg px-1.5 py-1">
      <Icon aria-hidden className="mt-px size-3 shrink-0" />
      <code className="min-w-0 font-mono text-[11px] break-all whitespace-pre-wrap select-text">{text}</code>
    </span>
  );
}
