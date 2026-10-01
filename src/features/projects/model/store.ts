import { useSyncExternalStore } from "react";
import { basename, folderNameError, joinPath, pathKey } from "./paths";
import type { Project } from "./project";

/**
 * Fake project store: nothing is created or linked on disk yet. Kept in memory
 * and mirrored to localStorage so projects survive reloads during development.
 * Swap the bodies of `createRepository` / `linkFolder` for backend commands later;
 * the hooks and result shapes can stay the same.
 */

const STORAGE_KEY = "foundry:projects";

export type ProjectResult = { ok: true; project: Project } | { ok: false; error: string };

function load(): readonly Project[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? (parsed as Project[]) : [];
  } catch {
    return [];
  }
}

let projects: readonly Project[] = load();
const listeners = new Set<() => void>();

function commit(next: readonly Project[]) {
  projects = next;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // Storage unavailable; projects just won't survive a reload.
  }
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

const getSnapshot = () => projects;

export function useProjects(): readonly Project[] {
  return useSyncExternalStore(subscribe, getSnapshot);
}

export function useProject(id: string | undefined): Project | undefined {
  const all = useProjects();
  return id === undefined ? undefined : all.find((project) => project.id === id);
}

function slugify(name: string): string {
  const slug = name
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || "project";
}

function uniqueId(name: string): string {
  const base = slugify(name);
  const taken = new Set(projects.map((project) => project.id));
  if (!taken.has(base)) return base;
  let n = 2;
  while (taken.has(`${base}-${n}`)) n += 1;
  return `${base}-${n}`;
}

function add(name: string, path: string, origin: Project["origin"]): ProjectResult {
  const key = pathKey(path);
  const existing = projects.find((project) => pathKey(project.path) === key);
  if (existing) return { ok: false, error: `Already added as “${existing.name}”.` };

  const project: Project = { id: uniqueId(name), name, path, origin, createdAt: Date.now() };
  commit([...projects, project]);
  return { ok: true, project };
}

/** Fake: records a new repository at `parentPath/name` without touching the disk. */
export function createRepository(name: string, parentPath: string): ProjectResult {
  const error = folderNameError(name);
  if (error) return { ok: false, error };
  if (!parentPath) return { ok: false, error: "Choose a parent folder." };
  return add(name, joinPath(parentPath, name), "created");
}

/** Fake: records an existing folder as a project. */
export function linkFolder(path: string): ProjectResult {
  if (!path) return { ok: false, error: "Choose a folder." };
  return add(basename(path), path, "linked");
}
