import { cn } from "cn";
import type { SaveStatus as Status } from "../hooks/autosave";

const LABELS = { saving: "Saving…", saved: "Saved" } as const;

/** "Saving…", "Saved", or the backend's error message as-is. Nothing before the first edit. */
export function SaveStatus({ status }: { status: Status }) {
  if (status.state === "idle") return null;
  const error = status.state === "error";
  const text = error ? status.message : LABELS[status.state];

  return (
    <span
      // Re-keyed per state so each change fades in; polite so typing isn't interrupted.
      key={error ? `error:${text}` : status.state}
      role={error ? "alert" : "status"}
      aria-live={error ? "assertive" : "polite"}
      title={text}
      className={cn(
        "block max-w-[min(32rem,60vw)] truncate text-[12px] tabular-nums animate-in fade-in duration-200 motion-reduce:animate-none",
        error ? "text-destructive" : "text-muted-foreground",
      )}
    >
      {text}
    </span>
  );
}
