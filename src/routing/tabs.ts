import type { IconType } from "@/shared";
import { humanize } from "./humanize";
import { pages } from "./pages";

/**
 * Optional metadata for a tab, as the default export of a `tab.ts` next to its
 * page. Import `TabMeta` with `import type` so the file stays free of runtime
 * dependencies:
 *
 *   import type { TabMeta } from "@/routing";
 *   export default { label: "Home", icon: House, order: 0 } satisfies TabMeta;
 */
export interface TabMeta {
  /** Defaults to the folder name, e.g. "project-settings" -> "Project settings". */
  label?: string;
  icon?: IconType;
  /** Lower comes first. Tabs without one go last, then sort by label. */
  order?: number;
}

export interface Tab {
  /** Folder name; stable and unique. */
  id: string;
  path: string;
  label: string;
  icon?: IconType;
  order: number;
  /** Starts downloading the page's chunk, e.g. on hover, so navigation is instant. */
  preload: () => void;
}

// Eager: metadata is tiny and needed to render the sidebar. Pages stay lazy.
const metaModules = import.meta.glob<TabMeta>("../pages/app/tabs/*/tab.ts", {
  eager: true,
  import: "default",
});

export const tabs: readonly Tab[] = pages
  .flatMap((page) => {
    if (!page.tab) return [];
    const meta = metaModules[`../pages/app/tabs/${page.tab}/tab.ts`] ?? {};
    const tab: Tab = {
      id: page.tab,
      path: page.path,
      label: meta.label ?? humanize(page.tab),
      icon: meta.icon,
      order: meta.order ?? Number.POSITIVE_INFINITY,
      preload: () => {
        // Errors resurface when the route itself loads the chunk.
        page.load().catch(() => {});
      },
    };
    return [tab];
  })
  .sort((a, b) => a.order - b.order || a.label.localeCompare(b.label));
