import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useState,
  type Dispatch,
  type ReactNode,
  type SetStateAction,
} from "react";

/** One step in the page header's trail. */
export interface Crumb {
  title: string;
  /** Muted text beside the title, e.g. a project's tagline. */
  subtitle?: string;
  /** Makes the crumb a link. Ignored on the last crumb (the current page). */
  to?: string;
}

type Entry = { owner: string; crumbs: readonly Crumb[] };

// Split so pages that only publish never re-render when the header changes.
const CrumbsContext = createContext<readonly Crumb[] | null>(null);
const SetEntryContext = createContext<Dispatch<SetStateAction<Entry | null>> | null>(null);

export function PageHeaderProvider({ children }: { children: ReactNode }) {
  const [entry, setEntry] = useState<Entry | null>(null);

  return (
    <SetEntryContext.Provider value={setEntry}>
      <CrumbsContext.Provider value={entry?.crumbs ?? null}>{children}</CrumbsContext.Provider>
    </SetEntryContext.Provider>
  );
}

/**
 * Sets the header while the calling component is mounted. Pass one crumb for
 * a title + subtitle, or several for breadcrumbs:
 *
 *   usePageHeader({ title: project.name, subtitle: project.tagline });
 *   usePageHeader([
 *     { title: project.name, subtitle: project.tagline, to: `/projects/${id}` },
 *     { title: "Blueprint" },
 *   ]);
 *
 * Inline literals are fine: crumbs are compared by value, not identity.
 */
export function usePageHeader(header: Crumb | readonly Crumb[]): void {
  const setEntry = useContext(SetEntryContext);
  if (!setEntry) {
    throw new Error("usePageHeader must be used within a PageHeaderProvider.");
  }

  const owner = useId();
  const key = JSON.stringify(([] as Crumb[]).concat(header));

  // Layout effect: the header updates in the same frame as the page, no flash.
  useLayoutEffect(() => {
    setEntry({ owner, crumbs: JSON.parse(key) as Crumb[] });
    // Only clear if a newer page hasn't already taken over.
    return () => setEntry((current) => (current?.owner === owner ? null : current));
  }, [setEntry, owner, key]);
}

/** Crumbs set by the current page, or `null` if it hasn't set any. */
export function usePageHeaderCrumbs(): readonly Crumb[] | null {
  return useContext(CrumbsContext);
}
