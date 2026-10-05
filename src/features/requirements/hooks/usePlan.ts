import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AppError, type Plan, type PlanAdded, type PlanSummary } from "@/lib/bindings";
import { call, type Outcome } from "./call";

/**
 * The project's plan and its summary, refetched on `plan:changed`. Mutations go through
 * `mutate`, which passes the last revision seen as `expectedRevision`, runs one at a time
 * (so our own quick edits never race each other into `stale_revision`), and adopts the plan
 * the backend returns. `stale_revision` refetches and raises a notice.
 *
 * Same shape as `features/blueprint/hooks/usePlan.ts`: features fetch their own data.
 */

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

export type PlanLoad =
  | { status: "loading" }
  | { status: "error"; error: AppError }
  | { status: "ready"; plan: Plan; summary: PlanSummary | null };

/** Shown above the page: the plan was reloaded after `stale_revision`, or an action failed. */
export interface PlanNotice {
  kind: "stale" | "error";
  /** The backend's message, as-is. */
  message: string;
  /** Distinguishes two equal notices in a row (restarts the dismiss timer). */
  id: number;
}

export interface PlanController {
  load: PlanLoad;
  notice: PlanNotice | null;
  dismissNotice: () => void;
  /** Shows a failed action's message in the page notice. */
  report: (error: AppError) => void;
  mutate: <T extends Plan | PlanAdded>(run: (revision: number) => Promise<Result<T>>) => Promise<Outcome<T>>;
}

const planOf = (data: Plan | PlanAdded): Plan => ("plan" in data ? data.plan : data);

export function isStale(outcome: Outcome<unknown>): boolean {
  return !outcome.ok && outcome.error.code === "stale_revision";
}

export function usePlan(projectId: string): PlanController {
  const [load, setLoad] = useState<PlanLoad>({ status: "loading" });
  const [notice, setNotice] = useState<PlanNotice | null>(null);
  const planRef = useRef<Plan | null>(null);
  const latestFetch = useRef(0);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const noticeId = useRef(0);

  /** Adopts a plan unless an older one arrived late (revisions only go up). */
  const adopt = useCallback((plan: Plan, summary?: PlanSummary | null) => {
    const current = planRef.current;
    if (current && plan.revision < current.revision) return;
    planRef.current = plan;
    setLoad((previous) => ({
      status: "ready",
      plan,
      summary: summary !== undefined ? summary : previous.status === "ready" ? previous.summary : null,
    }));
  }, []);

  const refetch = useCallback(async () => {
    const request = ++latestFetch.current;
    const [plan, summary] = await Promise.all([
      call(() => commands.getPlan(projectId)),
      call(() => commands.planSummary(projectId)),
    ]);
    if (request !== latestFetch.current) return;
    if (plan.ok) adopt(plan.data, summary.ok ? summary.data : null);
    // A failed refresh keeps showing what was there; only a failed first load is an error page.
    else if (!planRef.current) setLoad({ status: "error", error: plan.error });
    else setNotice({ kind: "error", message: plan.error.message, id: ++noticeId.current });
  }, [projectId, adopt]);

  const refreshSummary = useCallback(async () => {
    const summary = await call(() => commands.planSummary(projectId));
    if (summary.ok) setLoad((previous) => (previous.status === "ready" ? { ...previous, summary: summary.data } : previous));
  }, [projectId]);

  // The widget is keyed by project, so this runs once per project: no state to reset here.
  useEffect(() => {
    void refetch();

    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.planChanged
      .listen(({ payload }) => {
        if (payload.projectId !== projectId) return;
        // Our own mutation already adopted this revision; just the summary may be behind.
        if (payload.revision === planRef.current?.revision) void refreshSummary();
        else void refetch();
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [projectId, refetch, refreshSummary]);

  const mutate = useCallback(
    <T extends Plan | PlanAdded>(run: (revision: number) => Promise<Result<T>>): Promise<Outcome<T>> => {
      const next = queue.current.then(async (): Promise<Outcome<T>> => {
        const plan = planRef.current;
        if (!plan) return { ok: false, error: { code: "internal", message: "The plan hasn't loaded yet." } };
        const outcome = await call(() => run(plan.revision));
        if (outcome.ok) {
          adopt(planOf(outcome.data));
          void refreshSummary();
        } else if (outcome.error.code === "stale_revision") {
          setNotice({ kind: "stale", message: outcome.error.message, id: ++noticeId.current });
          await refetch();
        }
        return outcome;
      });
      queue.current = next.catch(() => {});
      return next;
    },
    [adopt, refetch, refreshSummary],
  );

  const report = useCallback((error: AppError) => {
    setNotice({ kind: error.code === "stale_revision" ? "stale" : "error", message: error.message, id: ++noticeId.current });
  }, []);

  const dismissNotice = useCallback(() => setNotice(null), []);

  return { load, notice, dismissNotice, report, mutate };
}
