import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";
import { DEFAULT_AVATAR } from "../lib/avatar";
import type { UnreadWorst } from "../lib/events";

const DOT: Record<Exclude<UnreadWorst, null>, { className: string; label: string }> = {
  danger: { className: "bg-destructive", label: "Unread danger" },
  warn: { className: "bg-warning", label: "Unread warning" },
  info: { className: "bg-muted-foreground/70", label: "Unread events" },
};

/** An agent's avatar (the default one when it has none or it fails), with an optional unread dot. */
export function AgentAvatar({ src, dot = null, className }: { src: string; dot?: UnreadWorst; className?: string }) {
  return (
    <span className={cn("relative inline-flex shrink-0", className)}>
      <Avatar size="sm" className="size-full">
        <AvatarImage src={src} alt="" />
        <AvatarFallback>
          <img src={DEFAULT_AVATAR} alt="" className="size-full rounded-full object-cover" />
        </AvatarFallback>
      </Avatar>
      {dot && (
        <span
          role="img"
          aria-label={DOT[dot].label}
          className={cn("absolute -top-px -right-px z-10 size-[7px] rounded-full ring-[1.5px] ring-sidebar", DOT[dot].className)}
        />
      )}
    </span>
  );
}
