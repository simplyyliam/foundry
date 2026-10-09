import { useEffect, useSyncExternalStore } from "react";
import { commands, events, type AppError, type ErrorCode, type Verification, type VerifyStage } from "@/lib/bindings";
import { call, type Outcome } from "./call";

/**
 * Verification runs, one state per project, kept outside React so a run survives leaving the
 * page. `verify:progress`, `verify:skip_cleared` and `agent:event` are listened to from app start
 * (`useVerificationListeners`, mounted by the `AutoVerify` widget in the app layout).
 *
 * Auto mode: when `defaultVerifyMode` is `auto` and an agent reports `report_ready`, that slice
 * is queued and verified. Nothing else ever starts a verification on its own.
 */

export interface RunProgress {
  sliceId: string;
  stage: VerifyStage;
  /** The test command running, with `running_tests`. */
  command: string | null;
  /** From the last event. */
  elapsedMs: number;
  /** When that event arrived (Date.now()), to count up locally between events. */
  receivedAt: number;
  /** Set from an event (vs. shown right after a click, before the first event). */
  fromEvent: boolean;
}

export interface ProjectRuns {
  running: RunProgress | null;
  /** Per slice: the last refused or failed Verify, exactly as the backend returned it. */
  errors: Readonly<Record<string, AppError>>;
  /** Goes up whenever a run ends, so views read reports and results again. */
  finished: number;
}

export interface AutoNotice {
  id: number;
  projectId: string;
  /** `commands`: the test commands need confirming, so nothing ran. */
  kind: "commands" | "error";
  /** The backend's message, as-is. */
  message: string;
}

const IDLE: ProjectRuns = { running: null, errors: {}, finished: 0 };
const TERMINAL: ReadonlySet<VerifyStage> = new Set(["done", "failed", "cancelled"]);
/** Refusals the backend records as the slice's skip (`SkipReason`), so `verify:skip_cleared` ends them. */
const SKIP_CODES: ReadonlySet<ErrorCode> = new Set([
  "test_commands_unconfirmed",
  "not_found",
  "invalid_json",
  "no_start_commit",
  "verification_running",
  "setup_not_committed",
]);

const states = new Map<string, ProjectRuns>();
const listeners = new Set<() => void>();
let notices: readonly AutoNotice[] = [];
let noticeId = 0;

const get = (projectId: string) => states.get(projectId) ?? IDLE;

function update(projectId: string, change: (previous: ProjectRuns) => Partial<ProjectRuns>) {
  const previous = get(projectId);
  states.set(projectId, { ...previous, ...change(previous) });
  listeners.forEach((listener) => listener());
}

function setNotices(next: readonly AutoNotice[]) {
  notices = next;
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function withoutError(errors: ProjectRuns["errors"], sliceId: string) {
  const { [sliceId]: _removed, ...rest } = errors;
  return rest;
}

/**
 * Verifies one slice. Every refusal is kept as-is for that slice's row; nothing is refused
 * here. Resolves when the run ends (it can take minutes).
 */
export async function verify(projectId: string, sliceId: string): Promise<Outcome<Verification>> {
  update(projectId, (previous) => ({
    errors: withoutError(previous.errors, sliceId),
    // Show it as starting right away, unless another run is known to be going.
    running: previous.running ?? {
      sliceId,
      stage: "starting",
      command: null,
      elapsedMs: 0,
      receivedAt: Date.now(),
      fromEvent: false,
    },
  }));

  const outcome = await call(() => commands.verifySlice(projectId, sliceId));

  update(projectId, (previous) => {
    const refusedAsBusy = !outcome.ok && outcome.error.code === "verification_running";
    const ours = previous.running?.sliceId === sliceId;
    // Busy: only our own "starting" placeholder goes; the other run keeps showing.
    const clear = ours && (!refusedAsBusy || !previous.running!.fromEvent);
    return {
      running: clear ? null : previous.running,
      errors: outcome.ok ? previous.errors : { ...previous.errors, [sliceId]: outcome.error },
      finished: refusedAsBusy ? previous.finished : previous.finished + 1,
    };
  });
  return outcome;
}

/** Stops the project's running verification; it then ends as `cancelled`. */
export async function cancelVerification(projectId: string): Promise<Outcome<boolean>> {
  return call(() => commands.cancelVerification(projectId));
}

export function clearVerifyError(projectId: string, sliceId: string) {
  update(projectId, (previous) => ({ errors: withoutError(previous.errors, sliceId) }));
}

/* ---------- auto mode ---------- */

/** Slices waiting to be verified automatically, per project, oldest first. */
const queues = new Map<string, string[]>();
const pumping = new Set<string>();
/** Runs that ended per project, to spot one ending while our request was in flight. */
const ended = new Map<string, number>();

function notify(projectId: string, kind: AutoNotice["kind"], message: string) {
  // One notice per project and kind: a newer one replaces it.
  const others = notices.filter((notice) => !(notice.projectId === projectId && notice.kind === kind));
  setNotices([...others, { id: ++noticeId, projectId, kind, message }]);
}

export function dismissAutoNotice(id: number) {
  setNotices(notices.filter((notice) => notice.id !== id));
}

async function onReportReady(projectId: string, sliceId: string) {
  // Read now, not cached: a change in Settings applies to the very next report.
  const settings = await call(() => commands.getSettings());
  if (!settings.ok) {
    notify(projectId, "error", settings.error.message);
    return;
  }
  if (settings.data.defaultVerifyMode !== "auto") return;
  const queue = queues.get(projectId) ?? [];
  if (!queue.includes(sliceId)) queues.set(projectId, [...queue, sliceId]);
  void pump(projectId);
}

/**
 * Verifies the project's queued slices one at a time. When another verification is running
 * (`verification_running`), it stops and runs again on that run's `done`, `failed` or
 * `cancelled`.
 */
async function pump(projectId: string) {
  if (pumping.has(projectId)) return;
  pumping.add(projectId);
  try {
    for (;;) {
      const queue = queues.get(projectId) ?? [];
      const sliceId = queue[0];
      if (sliceId === undefined) return;
      const endedBefore = ended.get(projectId) ?? 0;
      const outcome = await verify(projectId, sliceId);
      if (!outcome.ok && outcome.error.code === "verification_running") {
        // That run already ended while we asked: try again now. Otherwise wait for its end.
        if ((ended.get(projectId) ?? 0) !== endedBefore) continue;
        return;
      }
      queues.set(projectId, (queues.get(projectId) ?? []).filter((queued) => queued !== sliceId));
      if (outcome.ok) continue;
      const code = outcome.error.code;
      if (code === "test_commands_unconfirmed" || code === "test_commands_changed") {
        // Don't run anything until the commands are confirmed again.
        queues.delete(projectId);
        notify(projectId, "commands", outcome.error.message);
        return;
      }
      notify(projectId, "error", outcome.error.message);
    }
  } finally {
    pumping.delete(projectId);
  }
}

/* ---------- listeners ---------- */

let listening = false;

/** Starts the app-wide `verify:progress` and `agent:event` listeners once. */
export function useVerificationListeners() {
  useEffect(() => {
    if (listening) return;
    listening = true;
    events.verifyProgress
      .listen(({ payload }) => {
        const { projectId, sliceId, stage, command, elapsedMs } = payload;
        if (TERMINAL.has(stage)) {
          ended.set(projectId, (ended.get(projectId) ?? 0) + 1);
          update(projectId, (previous) => ({
            running: null,
            finished: previous.finished + 1,
            // "Already running" no longer holds once that run has ended.
            errors: Object.fromEntries(
              Object.entries(previous.errors).filter(([, error]) => error.code !== "verification_running"),
            ),
          }));
          void pump(projectId);
          return;
        }
        update(projectId, () => ({
          running: { sliceId, stage, command, elapsedMs, receivedAt: Date.now(), fromEvent: true },
        }));
      })
      .catch((error: unknown) => console.error("verify:progress listener failed", error));
    events.verifySkipCleared
      .listen(({ payload }) => {
        // The refusal no longer holds: the row shouldn't keep saying it does.
        const error = get(payload.projectId).errors[payload.sliceId];
        if (error && SKIP_CODES.has(error.code)) clearVerifyError(payload.projectId, payload.sliceId);
      })
      .catch((error: unknown) => console.error("verify:skip_cleared listener failed", error));
    events.agentEvent
      .listen(({ payload }) => {
        if (payload.kind === "report_ready" && payload.sliceId) void onReportReady(payload.projectId, payload.sliceId);
      })
      .catch((error: unknown) => console.error("agent:event listener failed", error));
  }, []);
}

export function useVerifyRuns(projectId: string): ProjectRuns {
  return useSyncExternalStore(subscribe, () => get(projectId));
}

export function useAutoNotices(): readonly AutoNotice[] {
  return useSyncExternalStore(subscribe, () => notices);
}
