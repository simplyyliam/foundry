import { useEffect, useSyncExternalStore } from "react";
import { commands, events, type AppError, type Plan, type Stage } from "@/lib/bindings";
import { call } from "./call";
import { refetchRoadmap } from "./useRoadmap";

/**
 * Roadmap generation, one state per project, kept outside React so a run survives switching
 * panel views or pages. `roadmap:progress` is listened to from app start
 * (`useRoadmapProgressListener` in the app layout), not only while a Roadmap view is mounted.
 *
 * Flow: getPlan → (confirmed requirements? ask) → generateRoadmap → getPlan again →
 * loadRoadmap(discardConfirmed, that revision). Nothing generates without an explicit click.
 */

export type GenerationPhase = "idle" | "checking" | "confirming" | "generating" | "loading";

export interface GenerationProgress {
  stage: Stage;
  /** From the last event. */
  elapsedMs: number;
  /** When that event arrived (Date.now()), to count up locally between events. */
  receivedAt: number;
}

export interface DiscardPrompt {
  /** Confirmed requirements that loading would discard. */
  count: number;
  /** Asked before generating, or after (the plan changed, or load_roadmap refused). */
  when: "before-generate" | "before-load";
}

export interface GenerationState {
  phase: GenerationPhase;
  prompt: DiscardPrompt | null;
  progress: GenerationProgress | null;
  /** The last failure, exactly as the backend returned it. */
  errors: AppError[];
  /** This window started the run (vs. one only seen through events, e.g. after a reload). */
  ownRun: boolean;
}

const IDLE: GenerationState = { phase: "idle", prompt: null, progress: null, errors: [], ownRun: false };

const states = new Map<string, GenerationState>();
const answers = new Map<string, (discard: boolean) => void>();
const listeners = new Set<() => void>();

const get = (projectId: string) => states.get(projectId) ?? IDLE;

function update(projectId: string, patch: Partial<GenerationState>) {
  states.set(projectId, { ...get(projectId), ...patch });
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export const isBusy = (state: GenerationState) => state.phase !== "idle";

const confirmedCount = (plan: Plan) =>
  plan.requirements.filter((requirement) => requirement.state === "confirmed").length;

function finish(projectId: string, errors: AppError[]) {
  update(projectId, { phase: "idle", prompt: null, progress: null, errors, ownRun: false });
  void refetchRoadmap(projectId);
}

/** Shows the discard dialog and waits for the answer (true = discard and continue). */
function ask(projectId: string, prompt: DiscardPrompt): Promise<boolean> {
  update(projectId, { phase: "confirming", prompt });
  return new Promise((resolve) => answers.set(projectId, resolve));
}

/** Answers the discard dialog. `false` stops here and changes nothing. */
export function answerDiscard(projectId: string, discard: boolean) {
  const resolve = answers.get(projectId);
  answers.delete(projectId);
  resolve?.(discard);
}

/** Generate or Regenerate. Ignored while this project is already busy. */
export async function generate(projectId: string): Promise<void> {
  if (isBusy(get(projectId))) return;
  update(projectId, { phase: "checking", errors: [], progress: null, ownRun: true });

  const before = await call(() => commands.getPlan(projectId));
  if (!before.ok) return finish(projectId, [before.error]);

  // How many confirmed requirements the user agreed to discard.
  let approved = 0;
  const confirmed = confirmedCount(before.data);
  if (confirmed > 0) {
    if (!(await ask(projectId, { count: confirmed, when: "before-generate" }))) return finish(projectId, []);
    approved = confirmed;
  }

  update(projectId, { phase: "generating", prompt: null, progress: null });
  const generated = await call(() => commands.generateRoadmap(projectId));
  // A failed generation leaves the plan untouched; show why.
  if (!generated.ok) return finish(projectId, [generated.error]);

  // Load into the plan. Re-read the plan first: it may have changed during the run, and the
  // user must agree to every confirmed requirement that would be discarded.
  for (let attempt = 0; attempt < 3; attempt += 1) {
    update(projectId, { phase: "loading", prompt: null });
    const current = await call(() => commands.getPlan(projectId));
    if (!current.ok) return finish(projectId, [current.error]);

    const now = confirmedCount(current.data);
    if (now > approved) {
      if (!(await ask(projectId, { count: now, when: "before-load" }))) return finish(projectId, []);
      approved = now;
      continue;
    }

    const loaded = await call(() =>
      commands.loadRoadmap(projectId, { discardConfirmed: approved > 0 }, current.data.revision),
    );
    if (loaded.ok) return finish(projectId, []);
    if (loaded.error.code !== "confirmed_requirements") return finish(projectId, [loaded.error]);
    // Requirements got confirmed between our read and the load: ask with the new count.
    const recount = await call(() => commands.getPlan(projectId));
    const count = recount.ok ? confirmedCount(recount.data) : now + 1;
    if (!(await ask(projectId, { count, when: "before-load" }))) return finish(projectId, []);
    approved = count;
  }
  finish(projectId, [{ code: "internal", message: "The plan kept changing while the roadmap was loading." }]);
}

/** Stops the running generation; it then fails with `cancelled`, shown like any failure. */
export async function cancelGeneration(projectId: string): Promise<void> {
  const outcome = await call(() => commands.cancelGeneration(projectId));
  if (!outcome.ok) update(projectId, { errors: [outcome.error] });
}

let listening = false;

/** Starts the app-wide `roadmap:progress` listener once. Call from the app layout. */
export function useRoadmapProgressListener() {
  useEffect(() => {
    if (listening) return;
    listening = true;
    void events.roadmapProgress
      .listen(({ payload }) => {
        const { projectId, stage, elapsedMs } = payload;
        const progress = { stage, elapsedMs, receivedAt: Date.now() };
        if (get(projectId).ownRun) {
          // Our own run: the generate() promise owns the phase; just show progress.
          update(projectId, { progress });
        } else if (stage === "done" || stage === "failed") {
          // A run we only saw through events (e.g. started before a reload) ended.
          update(projectId, { phase: "idle", progress: null });
          void refetchRoadmap(projectId);
        } else {
          update(projectId, { phase: "generating", progress });
        }
      })
      .catch(() => {
        listening = false;
      });
  }, []);
}

export function useGeneration(projectId: string): GenerationState {
  return useSyncExternalStore(subscribe, () => get(projectId));
}
