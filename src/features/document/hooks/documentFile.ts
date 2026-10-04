import { commands } from "@/lib/bindings";
import { call, type Outcome } from "./call";

/**
 * Reads and writes `docs/brain-dump.md`. Saves run one at a time, in order (an older save can
 * never land after a newer one), and a load waits for every queued save first. That is what
 * makes "switching project saves the old one before loading the new one" hold, and stops a
 * quick A → B → A from reading A before its last save landed.
 */

let queue: Promise<unknown> = Promise.resolve();

/**
 * Text whose save failed, per project. Kept in memory so leaving the page (which unmounts the
 * editor) never loses it: the next load of that project shows it again and retries.
 */
const unsaved = new Map<string, string>();

export function saveDocumentFile(projectId: string, markdown: string): Promise<Outcome<null>> {
  const save = queue.then(() => call(() => commands.saveDocument(projectId, markdown)));
  queue = save;
  return save.then((outcome) => {
    if (outcome.ok) unsaved.delete(projectId);
    else unsaved.set(projectId, markdown);
    return outcome;
  });
}

export interface LoadedDocument {
  /** The file as it is on disk. */
  file: string;
  /** Text from this session that failed to save, to show instead of `file`. */
  draft: string | null;
}

export async function loadDocumentFile(projectId: string): Promise<Outcome<LoadedDocument>> {
  await queue;
  const outcome = await call(() => commands.getDocument(projectId));
  if (!outcome.ok) return outcome;
  const draft = unsaved.get(projectId) ?? null;
  return { ok: true, data: { file: outcome.data, draft: draft === outcome.data ? null : draft } };
}
