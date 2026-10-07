import {
  CircleStop,
  ClipboardCheck,
  FileCheck2,
  FilePen,
  Hand,
  Hourglass,
  MessageSquare,
  Moon,
  OctagonAlert,
  Play,
  ShieldBan,
  SquareTerminal,
  type LucideIcon,
} from "lucide-react";
import type { EventKind } from "@/lib/bindings";

/** A small icon per event kind. */
export const KIND_ICONS: Record<EventKind, LucideIcon> = {
  session_started: Play,
  edit: FilePen,
  command: SquareTerminal,
  tests_ran: ClipboardCheck,
  blocked: ShieldBan,
  needs_attention: Hand,
  turn_complete: MessageSquare,
  session_ended: CircleStop,
  stuck: Hourglass,
  quiet: Moon,
  danger: OctagonAlert,
  report_ready: FileCheck2,
};
