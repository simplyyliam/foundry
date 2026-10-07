import { cn } from "@/lib/utils";
import { statusLabel, type SessionView } from "../lib/session";

/** A small dot and "not started" / "starting" / "running" / "exited (1)". */
export function SessionStatus({ session, className }: { session: SessionView | undefined; className?: string }) {
  const status = session?.status ?? "idle";
  const failed = status === "exited" && session?.exitCode !== null && session?.exitCode !== 0;
  return (
    <span className={cn("flex shrink-0 items-center gap-1.5 text-[11.5px] text-muted-foreground", className)}>
      <span
        aria-hidden
        className={cn(
          "size-1.5 rounded-full",
          status === "running" && "bg-status-running",
          status === "starting" && "animate-pulse bg-status-starting motion-reduce:animate-none",
          status === "exited" && (failed ? "bg-destructive" : "bg-muted-foreground/60"),
          status === "idle" && "bg-muted-foreground/35",
        )}
      />
      <span className="tabular-nums">{statusLabel(session)}</span>
    </span>
  );
}
