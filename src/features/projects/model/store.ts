import { useSyncExternalStore } from "react";
import {
  commands,
  type AppError,
  type CreateRepositoryResult,
  type LinkRepositoryResult,
  type Project,
} from "@/lib/bindings";

/**
 * Client cache of the backend project registry. `listProjects()` is the source of truth:
 * it loads on first use and re-runs after every change made here, so the sidebar always
 * shows what's on disk.
 */

export type Outcome<T> = { ok: true; data: T } | { ok: false; error: AppError };

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

export interface ProjectsSnapshot {
  /** "loading" only until the first response; refreshes keep showing the current list. */
  status: "loading" | "ready" | "error";
  projects: readonly Project[];
  error: AppError | null;
}

let snapshot: ProjectsSnapshot = { status: "loading", projects: [], error: null };
let started = false;
let latestRequest = 0;
const listeners = new Set<() => void>();

function publish(next: ProjectsSnapshot) {
  snapshot = next;
  listeners.forEach((listener) => listener());
}

/** Turns a binding call (which can also throw, e.g. outside Tauri) into an Outcome. */
async function call<T>(request: () => Promise<Result<T>>): Promise<Outcome<T>> {
  try {
    const result = await request();
    return result.status === "ok" ? { ok: true, data: result.data } : { ok: false, error: result.error };
  } catch (error) {
    // Not an AppError (IPC itself failed): pass its own message through.
    return { ok: false, error: { code: "internal", message: error instanceof Error ? error.message : String(error) } };
  }
}

/** Re-reads the registry. Responses that arrive out of order are dropped. */
export async function refreshProjects(): Promise<void> {
  const request = ++latestRequest;
  const outcome = await call(() => commands.listProjects());
  if (request !== latestRequest) return;
  publish(
    outcome.ok
      ? { status: "ready", projects: outcome.data, error: null }
      : { status: "error", projects: snapshot.projects, error: outcome.error },
  );
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  if (!started) {
    started = true;
    void refreshProjects();
  }
  return () => listeners.delete(listener);
}

const getSnapshot = () => snapshot;

export function useProjectsSnapshot(): ProjectsSnapshot {
  return useSyncExternalStore(subscribe, getSnapshot);
}

export function useProjects(): readonly Project[] {
  return useProjectsSnapshot().projects;
}

/** The project with `id`, and whether the first load has finished (so "missing" is real). */
export function useProject(id: string | undefined): { project: Project | undefined; loaded: boolean } {
  const { status, projects } = useProjectsSnapshot();
  return {
    project: id === undefined ? undefined : projects.find((project) => project.id === id),
    loaded: status !== "loading",
  };
}

/** Runs a mutation, then refreshes the list before resolving, so callers can navigate safely. */
async function mutate<T>(request: () => Promise<Result<T>>): Promise<Outcome<T>> {
  const outcome = await call(request);
  if (outcome.ok) await refreshProjects();
  return outcome;
}

export const createRepository = (name: string, parentDir: string) =>
  mutate<CreateRepositoryResult>(() => commands.createRepository({ name, parentDir }));

export const linkRepository = (folder: string) =>
  mutate<LinkRepositoryResult>(() => commands.linkRepository({ folder }));

export const renameProject = (id: string, name: string) =>
  mutate<Project>(() => commands.updateProject(id, { name, tagline: null }));

export const archiveProject = (id: string) => mutate<Project>(() => commands.archiveProject(id));

export const unarchiveProject = (id: string) => mutate<Project>(() => commands.unarchiveProject(id));

/** Makes the project the backend's active one and records lastOpened. */
export const openProject = (id: string) => mutate<Project>(() => commands.openProject(id));

/**
 * PLACEHOLDER: there's no backend command that deletes a repository yet. `removeProject`
 * only unregisters and never deletes files, so it is deliberately not called here.
 * Replace the body with `mutate(() => commands.deleteRepository(id))` once it exists.
 */
export async function deleteRepository(id: string): Promise<Outcome<null>> {
  void id;
  return {
    ok: false,
    error: {
      code: "internal",
      message: "Delete isn't available yet: the backend has no delete_repository command.",
    },
  };
}
