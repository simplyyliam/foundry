import type { AppError, CommitSliceResult, CommitStatus } from "@/lib/bindings";

export type CommitOutcome = { ok: true; data: CommitSliceResult } | { ok: false; error: AppError };

export type CommitStatusLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; value: CommitStatus };

/**
 * What the commit button, prompt and dialog need about one slice. A feature hook provides it
 * (only feature hooks call bindings): `commitStatus` for the slice, and whether an agent session
 * is live in the project.
 */
export interface SliceCommitController {
  status: CommitStatusLoad;
  /** An agent session in this project is starting or running (`commit_slice` refuses then). */
  sessionLive: boolean;
  /** Reads `commitStatus` (and the sessions) again; resolves once that read has landed. */
  refresh: () => Promise<void>;
  /** `commitSlice` with the message as given, or `null` for the generated one. */
  commit: (message: string | null) => Promise<CommitOutcome>;
}

export interface CommitAvailability {
  enabled: boolean;
  /** Why it's disabled (shown as the tooltip); null when enabled. */
  reason: string | null;
}

/** Enabled when there are files to commit and no session is live. */
export function commitAvailability({ status, sessionLive }: Pick<SliceCommitController, "status" | "sessionLive">): CommitAvailability {
  if (sessionLive) return { enabled: false, reason: "A session is still running" };
  if (status.status === "loading") return { enabled: false, reason: "Checking for changes…" };
  if (status.status === "error") return { enabled: false, reason: status.error.message };
  if (status.value.total === 0) return { enabled: false, reason: "Nothing to commit" };
  return { enabled: true, reason: null };
}
