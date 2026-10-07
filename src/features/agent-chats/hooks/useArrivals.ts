import { useState } from "react";

/**
 * Ids that weren't there when the list first showed: those arrived live (or from "Load
 * older") and get an entrance animation; everything on first show appears as is.
 */
export function useArrivals(ids: readonly number[], ready: boolean): (id: number) => boolean {
  const [initial, setInitial] = useState<ReadonlySet<number> | null>(null);
  if (ready && initial === null) setInitial(new Set(ids));
  return (id) => initial !== null && !initial.has(id);
}
