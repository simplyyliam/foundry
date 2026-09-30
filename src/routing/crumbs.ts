import { matchPath } from "react-router-dom";
import type { Crumb } from "@/shared";
import { humanize } from "./humanize";
import { pages } from "./pages";
import { tabs } from "./tabs";

function decodeSegment(segment: string): string {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

const hasAppPage = (path: string) =>
  pages.some((page) => page.section === "app" && matchPath(page.path, path) !== null);

/**
 * Fallback header for pages that don't call `usePageHeader`: one crumb per URL
 * segment, using tab labels where they exist. Segments that have a page of
 * their own become links.
 *
 *   /home          -> Home
 *   /home/details  -> Home › Details
 *
 * Dynamic segments show their raw value (e.g. an id), so pages behind them
 * should set a proper title with `usePageHeader`.
 */
export function getRouteCrumbs(pathname: string): Crumb[] {
  const segments = pathname.split("/").filter(Boolean);

  return segments.map((segment, index) => {
    const path = `/${segments.slice(0, index + 1).join("/")}`;
    const tab = tabs.find((candidate) => candidate.path === path);
    return {
      title: tab?.label ?? humanize(decodeSegment(segment)),
      to: hasAppPage(path) ? path : undefined,
    };
  });
}
