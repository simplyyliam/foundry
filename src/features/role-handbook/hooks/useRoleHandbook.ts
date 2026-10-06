import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type AppError, type HandbookRole, type HandbookRolePatch, type NewHandbookRole, type RolesImported } from "@/lib/bindings";
import { call, type Outcome } from "./call";

type RolesResult = Promise<{ status: "ok"; data: HandbookRole[] } | { status: "error"; error: AppError }>;

export interface RoleHandbookData {
  /** Null until the first answer. The backend seeds the starter roles on that first read. */
  roles: HandbookRole[] | null;
  /** The backend's message when the list couldn't be read. */
  loadError: string | null;
  reload: () => Promise<void>;
  add: (role: NewHandbookRole) => Promise<Outcome<HandbookRole[]>>;
  /** Only the non-null fields change. */
  update: (id: string, patch: Partial<HandbookRolePatch>) => Promise<Outcome<HandbookRole[]>>;
  remove: (id: string) => Promise<Outcome<HandbookRole[]>>;
  /**
   * One role per dropped `.md` file (all or nothing). Without `replace`, a role the handbook
   * already has is `role_exists`. Lists again afterwards either way.
   */
  addFromPaths: (paths: string[], replace: boolean) => Promise<Outcome<RolesImported>>;
}

const EMPTY_PATCH: HandbookRolePatch = {
  title: null,
  description: null,
  typicalWriteGlobs: null,
  typicalProtectedGlobs: null,
  skillTemplate: null,
};

/**
 * The app-wide Role Handbook (not per project). Changes run one at a time, in order; each
 * adopts the list it returns and then lists again.
 */
export function useRoleHandbook(): RoleHandbookData {
  const [roles, setRoles] = useState<HandbookRole[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const mounted = useRef(true);

  const enqueue = useCallback(<T,>(task: () => Promise<T>): Promise<T> => {
    const run = queue.current.then(task);
    queue.current = run.catch(() => undefined);
    return run;
  }, []);

  const fetchList = useCallback(async () => {
    const outcome = await call(() => commands.roleHandbookList());
    if (!mounted.current) return;
    if (outcome.ok) setRoles(outcome.data);
    setLoadError(outcome.ok ? null : outcome.error.message);
  }, []);

  const reload = useCallback(() => enqueue(fetchList), [enqueue, fetchList]);

  const mutate = useCallback(
    (request: () => RolesResult) =>
      enqueue(async () => {
        const outcome = await call(request);
        if (outcome.ok && mounted.current) setRoles(outcome.data);
        await fetchList();
        return outcome;
      }),
    [enqueue, fetchList],
  );

  useEffect(() => {
    mounted.current = true;
    void reload();
    return () => {
      mounted.current = false;
    };
  }, [reload]);

  const add = useCallback((role: NewHandbookRole) => mutate(() => commands.roleHandbookAdd(role)), [mutate]);
  const update = useCallback(
    (id: string, patch: Partial<HandbookRolePatch>) => mutate(() => commands.roleHandbookUpdate(id, { ...EMPTY_PATCH, ...patch })),
    [mutate],
  );
  const remove = useCallback((id: string) => mutate(() => commands.roleHandbookRemove(id)), [mutate]);
  const addFromPaths = useCallback(
    (paths: string[], replace: boolean) =>
      enqueue(async () => {
        // Returns ids, not the list, so there's nothing to adopt: just list again.
        const outcome = await call(() => commands.roleHandbookAddFromPaths(paths, replace));
        await fetchList();
        return outcome;
      }),
    [enqueue, fetchList],
  );

  return { roles, loadError, reload, add, update, remove, addFromPaths };
}
