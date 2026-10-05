import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type DesignFile, type DesignFilePatch, type DesignLibrary } from "@/lib/bindings";
import { call, type Outcome } from "./call";

export interface DesignLibraryData {
  /** Null until the first answer. */
  library: DesignLibrary | null;
  /** The backend's message when the list couldn't be read. */
  loadError: string | null;
  reload: () => Promise<void>;
  /** Adds `.md` files by absolute path (all or nothing). */
  add: (paths: string[]) => Promise<Outcome<DesignLibrary>>;
  /** Saves the given fields; the rest are taken from the file as last saved, when the save runs. */
  update: (id: string, change: Partial<DesignFilePatch>) => Promise<Outcome<DesignLibrary>>;
  remove: (id: string) => Promise<Outcome<DesignLibrary>>;
}

/**
 * The app-wide Design Library (not per project). Changes run one at a time, in order; each
 * adopts the library it returns and then lists again.
 */
export function useDesignLibrary(): DesignLibraryData {
  const [library, setLibrary] = useState<DesignLibrary | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const latest = useRef<DesignLibrary | null>(null);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const mounted = useRef(true);

  const adopt = useCallback((next: DesignLibrary) => {
    latest.current = next;
    if (mounted.current) setLibrary(next);
  }, []);

  const enqueue = useCallback(<T,>(task: () => Promise<T>): Promise<T> => {
    const run = queue.current.then(task);
    queue.current = run.catch(() => undefined);
    return run;
  }, []);

  const fetchList = useCallback(async () => {
    const outcome = await call(() => commands.designLibraryList());
    if (outcome.ok) adopt(outcome.data);
    if (mounted.current) setLoadError(outcome.ok ? null : outcome.error.message);
  }, [adopt]);

  const reload = useCallback(() => enqueue(fetchList), [enqueue, fetchList]);

  const mutate = useCallback(
    (request: () => ReturnType<typeof commands.designLibraryList> | null) =>
      enqueue(async (): Promise<Outcome<DesignLibrary>> => {
        const pending = request();
        if (!pending) return { ok: false, error: { code: "not_found", message: "That file is no longer in the library." } } as Outcome<DesignLibrary>;
        const outcome = await call(() => pending);
        if (outcome.ok) adopt(outcome.data);
        await fetchList();
        return outcome;
      }),
    [enqueue, adopt, fetchList],
  );

  useEffect(() => {
    mounted.current = true;
    void reload();
    return () => {
      mounted.current = false;
    };
  }, [reload]);

  const add = useCallback((paths: string[]) => mutate(() => commands.designLibraryAddFromPaths(paths)), [mutate]);

  const update = useCallback(
    (id: string, change: Partial<DesignFilePatch>) =>
      mutate(() => {
        const file: DesignFile | undefined = latest.current?.files.find((candidate) => candidate.id === id);
        if (!file) return null;
        return commands.designLibraryUpdate(id, { families: file.families, description: file.description, ...change });
      }),
    [mutate],
  );

  const remove = useCallback((id: string) => mutate(() => commands.designLibraryRemove(id)), [mutate]);

  return { library, loadError, reload, add, update, remove };
}
