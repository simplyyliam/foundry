import type { ComponentType } from "react";

/**
 * File-based page discovery. Single source of truth for the router and for
 * anything that navigates to pages (e.g. the sidebar tabs).
 *
 *   pages/app/users/[id]/page.tsx          -> /users/:id
 *   pages/app/tabs/home/page.tsx           -> /home        (sidebar tab)
 *   pages/standalone/docs/[...rest]/page.tsx -> /docs/*
 */

export type PageModule = { default: ComponentType };

/** Which layout the page renders in. */
export type PageSection = "app" | "standalone";

export interface PageEntry {
  /** Source file, relative to this module. */
  file: string;
  section: PageSection;
  /** Route path, e.g. "/users/:id". */
  path: string;
  /** Folder name if this is a top-level tab: pages/app/tabs/<tab>/page.tsx. */
  tab?: string;
  load: () => Promise<PageModule>;
}

/**
 * Folder under pages/app whose direct children become sidebar tabs. It only
 * groups files: it isn't part of the URL.
 */
export const TABS_DIR = "tabs";

const PAGE_FILE = /^\.\.\/pages\/(app|standalone)\/(?:(.+)\/)?page\.tsx$/;

const isDynamicSegment = (segment: string) => segment.startsWith("[");

function toRouteSegment(segment: string): string {
  if (/^\[\.\.\.[^\]]+\]$/.test(segment)) return "*";
  return segment.replace(/^\[([^\]]+)\]$/, ":$1");
}

function toPageEntry(file: string, load: () => Promise<PageModule>): PageEntry | null {
  const match = PAGE_FILE.exec(file);
  if (!match) return null;

  const section = match[1] as PageSection;
  const segments = match[2]?.split("/") ?? [];

  const inTabs = section === "app" && segments[0] === TABS_DIR;
  if (inTabs) segments.shift();

  // Only static, direct children of tabs/ are tabs; deeper pages are ordinary routes.
  const tab =
    inTabs && segments.length === 1 && !isDynamicSegment(segments[0]) ? segments[0] : undefined;

  return {
    file,
    section,
    path: `/${segments.map(toRouteSegment).join("/")}`,
    tab,
    load,
  };
}

function assertUniquePaths(entries: readonly PageEntry[]): void {
  const seen = new Map<string, string>();
  for (const { path, file } of entries) {
    const existing = seen.get(path);
    if (existing) {
      throw new Error(`[routing] "${existing}" and "${file}" both resolve to "${path}".`);
    }
    seen.set(path, file);
  }
}

// Lazy glob: each page becomes its own chunk, loaded on first navigation.
const modules = import.meta.glob<PageModule>("../pages/**/page.tsx");

export const pages: readonly PageEntry[] = Object.entries(modules)
  .map(([file, load]) => toPageEntry(file, load))
  .filter((entry): entry is PageEntry => entry !== null);

assertUniquePaths(pages);
