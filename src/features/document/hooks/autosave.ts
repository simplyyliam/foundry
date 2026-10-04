import type { MarkdownManager } from "@tiptap/markdown";
import type { Node } from "@tiptap/pm/model";
import { serializeDocument, type SourceMap } from "../lib/markdown-source";
import { saveDocumentFile } from "./documentFile";

export type SaveStatus =
  | { state: "idle" }
  | { state: "saving" }
  | { state: "saved" }
  | { state: "error"; message: string };

export const IDLE: SaveStatus = { state: "idle" };
const SAVING: SaveStatus = { state: "saving" };
const SAVED: SaveStatus = { state: "saved" };

/** After the last edit. */
const DEBOUNCE_MS = 800;

interface AutosaveOptions {
  projectId: string;
  manager: MarkdownManager;
  /** Describes the text the editor was loaded with. */
  map: SourceMap;
  /** What's on disk now. */
  file: string;
  onStatus: (status: SaveStatus) => void;
  /** A save landed: `map` now describes the file. */
  onSaved: (map: SourceMap) => void;
}

/**
 * Debounced autosave for one project's open document. Edits only record the latest doc (a
 * pointer, no work per keystroke); markdown is produced at flush time. A flush that would
 * write what's already on disk (or queued) writes nothing, so opening without editing never
 * touches the file. A failed save keeps the edit pending: the next edit, blur or flush retries.
 */
export class Autosave {
  private readonly options: AutosaveOptions;
  private map: SourceMap;
  private doc: Node | null = null;
  private dirty = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** What the file holds once every queued save has landed. */
  private queued: string;
  /** What the file is known to hold. */
  private confirmed: string;
  private latest: Promise<boolean> | null = null;

  constructor(options: AutosaveOptions) {
    this.options = options;
    this.map = options.map;
    this.queued = options.file;
    this.confirmed = options.file;
  }

  /** The document changed. */
  edited(doc: Node) {
    this.doc = doc;
    this.dirty = true;
    this.options.onStatus(SAVING);
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.flush(), DEBOUNCE_MS);
  }

  /** Saves a pending edit now. Resolves true once everything is on disk, false if a save failed. */
  flush(): Promise<boolean> {
    clearTimeout(this.timer);
    this.timer = undefined;
    if (!this.dirty || !this.doc) return this.latest ?? Promise.resolve(true);
    this.dirty = false;

    const { markdown, map } = serializeDocument(this.doc, this.map, this.options.manager);
    if (markdown === this.queued) {
      if (!this.latest) this.options.onStatus(this.queued === this.options.file ? IDLE : SAVED);
      return this.latest ?? Promise.resolve(true);
    }

    this.queued = markdown;
    const save: Promise<boolean> = saveDocumentFile(this.options.projectId, markdown).then((outcome) => {
      const isLatest = this.latest === save;
      if (isLatest) this.latest = null;
      if (outcome.ok) {
        this.confirmed = markdown;
        this.map = map;
        this.options.onSaved(map);
        if (isLatest && !this.dirty) this.options.onStatus(SAVED);
        return true;
      }
      // Keep the edit: the next edit, blur or flush writes it again.
      this.dirty = true;
      this.queued = this.confirmed;
      this.options.onStatus({ state: "error", message: outcome.error.message });
      return false;
    });
    this.latest = save;
    return save;
  }
}
