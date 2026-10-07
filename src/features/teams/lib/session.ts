import type { SessionStatus } from "@/lib/bindings";

/** What a card knows about its agent's session. No entry means "not started". */
export interface SessionView {
  status: SessionStatus;
  exitCode: number | null;
  /** Epoch ms of the last change; an older report never replaces a newer one. */
  since: number;
}

export const isLive = (session: SessionView | undefined) =>
  session?.status === "starting" || session?.status === "running";

/** "not started", "starting", "running", "exited (1)", or "exited" when no code was reported. */
export function statusLabel(session: SessionView | undefined): string {
  switch (session?.status) {
    case "starting":
      return "starting";
    case "running":
      return "running";
    case "exited":
      return session.exitCode === null ? "exited" : `exited (${session.exitCode})`;
    default:
      return "not started";
  }
}
