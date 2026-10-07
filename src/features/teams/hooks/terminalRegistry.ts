import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import "./terminal.css";
import { commands, events } from "@/lib/bindings";
import { decodeBase64 } from "../lib/base64";
import { TERMINAL_FONT, terminalTheme } from "../lib/terminalTheme";
import { call } from "./call";

/** How long the size must hold before it's sent to the session. */
const RESIZE_DEBOUNCE_MS = 150;

interface Entry {
  agentId: string;
  term: Terminal;
  fit: FitAddon;
  /** Holds xterm's DOM. Moved between the card and the expanded dialog, never re-created. */
  host: HTMLDivElement;
  opened: boolean;
  /** False while the snapshot is being read: live output is dropped meanwhile (see `fill`). */
  filled: boolean;
  fillRequest: number;
  /** The size last sent to the session, so an unchanged size isn't sent again. */
  sent: { cols: number; rows: number } | null;
  frame: number | undefined;
  timer: number | undefined;
  force: boolean;
  /** Keystrokes go out one at a time, in order. */
  writes: Promise<void>;
}

export interface RegistryCallbacks {
  /** Whether the agent's session can take input and resizes right now. */
  isLive: (agentId: string) => boolean;
  /** A backend error from a terminal call, as-is. */
  onError: (agentId: string, message: string) => void;
}

/**
 * Every agent's terminal for one project, alive until the page unmounts. Each is filled from
 * `agentSnapshot` when first used, then appended to from `agent:output`; typing goes to
 * `agentWrite`, and its size to `agentResize` (debounced, only when it changed or is forced).
 */
export class TerminalRegistry {
  private readonly entries = new Map<string, Entry>();
  private disposed = false;
  private unlisten: (() => void) | undefined;
  private readonly themeObserver: MutationObserver;
  private themeKey = "";

  private readonly projectId: string;
  private readonly callbacks: RegistryCallbacks;

  constructor(projectId: string, callbacks: RegistryCallbacks) {
    this.projectId = projectId;
    this.callbacks = callbacks;
    events.agentOutput
      .listen(({ payload }) => {
        if (payload.projectId !== this.projectId) return;
        const entry = this.entries.get(payload.agentId);
        // Not created yet: the snapshot will hold this output when it is.
        if (!entry?.filled) return;
        // Bytes, not text: xterm keeps a UTF-8 sequence split across chunks intact.
        entry.term.write(decodeBase64(payload.chunk));
      })
      .then((stop) => {
        if (this.disposed) stop();
        else this.unlisten = stop;
      })
      .catch((error) => console.warn("Couldn't listen to agent output:", error));

    this.themeObserver = new MutationObserver(() => this.applyTheme());
    this.themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
  }

  private ensure(agentId: string): Entry {
    const existing = this.entries.get(agentId);
    if (existing) return existing;

    const theme = terminalTheme();
    this.themeKey = JSON.stringify(theme);
    const term = new Terminal({
      allowTransparency: true,
      fontFamily: TERMINAL_FONT,
      cursorBlink: true,
      scrollback: 5000,
      theme,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);

    const host = document.createElement("div");
    host.className = "teams-terminal-host";
    host.style.position = "absolute";
    host.style.inset = "0";

    const entry: Entry = {
      agentId,
      term,
      fit,
      host,
      opened: false,
      filled: false,
      fillRequest: 0,
      sent: null,
      frame: undefined,
      timer: undefined,
      force: false,
      writes: Promise.resolve(),
    };

    term.onData((data) => this.send(entry, data));
    term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") return true;
      const mod = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();
      // Ctrl+C copies when something is selected (like Windows Terminal); otherwise it interrupts.
      if (mod && !event.shiftKey && !event.altKey && key === "c" && term.hasSelection()) {
        void navigator.clipboard.writeText(term.getSelection()).catch((error) => console.warn("Copy failed:", error));
        term.clearSelection();
        return false;
      }
      // Let the browser paste: xterm's paste handler then sends the text (bracketed if asked for).
      if (mod && !event.altKey && key === "v") return false;
      return true;
    });

    this.entries.set(agentId, entry);
    this.fill(entry);
    return entry;
  }

  /**
   * Reads the snapshot into an empty terminal. Output that arrives while it's being read is
   * dropped: it was almost certainly emitted before the read, so the snapshot holds it.
   * (There's no sequence number to tell for sure; see DECISIONS.md.)
   */
  private fill(entry: Entry) {
    const request = ++entry.fillRequest;
    entry.filled = false;
    void call(() => commands.agentSnapshot(this.projectId, entry.agentId)).then((outcome) => {
      if (this.disposed || request !== entry.fillRequest) return;
      if (!outcome.ok) this.callbacks.onError(entry.agentId, outcome.error.message);
      else if (outcome.data) entry.term.write(decodeBase64(outcome.data));
      entry.filled = true;
    });
  }

  private send(entry: Entry, data: string) {
    if (!this.callbacks.isLive(entry.agentId)) return;
    entry.writes = entry.writes.then(async () => {
      const outcome = await call(() => commands.agentWrite(this.projectId, entry.agentId, data));
      if (!outcome.ok && !this.disposed) this.callbacks.onError(entry.agentId, outcome.error.message);
    });
  }

  private applyTheme() {
    if (this.entries.size === 0) return;
    const theme = terminalTheme();
    const key = JSON.stringify(theme);
    if (key === this.themeKey) return;
    this.themeKey = key;
    for (const entry of this.entries.values()) entry.term.options.theme = theme;
  }

  /** Shows the agent's terminal in `container` (moving it from wherever it was). */
  attach(agentId: string, container: HTMLElement) {
    if (this.disposed) return;
    const entry = this.ensure(agentId);
    container.appendChild(entry.host);
    if (!entry.opened) {
      // Opened only once it's in the document, so xterm can measure the font.
      entry.term.open(entry.host);
      entry.opened = true;
      this.useWebgl(entry);
    }
    this.scheduleResize(agentId);
  }

  /**
   * Draws with WebGL when it's available. Its renderer draws block and box characters itself,
   * on whole pixels, so drawings made of them (Claude Code's banner) have no seams; the DOM
   * renderer uses the font's glyphs, which leave gaps at fractional cell widths. If WebGL is
   * missing or its context is lost, the terminal falls back to the DOM renderer.
   */
  private useWebgl(entry: Entry) {
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      entry.term.loadAddon(webgl);
    } catch (error) {
      console.warn("WebGL terminal renderer unavailable, using the DOM renderer:", error);
    }
  }

  /** Takes the terminal out of `container`, unless it has already moved elsewhere. */
  detach(agentId: string, container: HTMLElement) {
    const entry = this.entries.get(agentId);
    if (entry && entry.host.parentElement === container) container.removeChild(entry.host);
  }

  /**
   * Fits the terminal to its container on the next frame, then sends the size to the session
   * once it has held for a moment. `force` sends it even if unchanged (a new session starts
   * at 120x30, whatever was sent to the last one).
   */
  scheduleResize(agentId: string, force = false) {
    const entry = this.entries.get(agentId);
    if (!entry || this.disposed) return;
    entry.force ||= force;
    if (entry.frame === undefined) {
      entry.frame = requestAnimationFrame(() => {
        entry.frame = undefined;
        this.fitNow(entry);
      });
    }
    window.clearTimeout(entry.timer);
    entry.timer = window.setTimeout(() => this.sendSize(entry), RESIZE_DEBOUNCE_MS);
  }

  private fitNow(entry: Entry): boolean {
    if (this.disposed || !entry.opened) return false;
    const parent = entry.host.parentElement;
    if (!parent || parent.clientWidth === 0 || parent.clientHeight === 0) return false;
    const size = entry.fit.proposeDimensions();
    if (!size || !Number.isFinite(size.cols) || !Number.isFinite(size.rows) || size.cols < 2 || size.rows < 1) return false;
    if (size.cols !== entry.term.cols || size.rows !== entry.term.rows) entry.term.resize(size.cols, size.rows);
    return true;
  }

  private sendSize(entry: Entry) {
    entry.timer = undefined;
    if (!this.fitNow(entry) || !this.callbacks.isLive(entry.agentId)) return;
    const { cols, rows } = entry.term;
    if (!entry.force && entry.sent?.cols === cols && entry.sent?.rows === rows) return;
    entry.force = false;
    entry.sent = { cols, rows };
    void call(() => commands.agentResize(this.projectId, entry.agentId, cols, rows)).then((outcome) => {
      if (outcome.ok || this.disposed) return;
      entry.sent = null;
      this.callbacks.onError(entry.agentId, outcome.error.message);
    });
  }

  /** Clears the terminal for a new session (its output then appends to an empty screen). */
  reset(agentId: string) {
    const entry = this.ensure(agentId);
    entry.fillRequest++;
    entry.term.reset();
    entry.filled = true;
    entry.sent = null;
  }

  /** Clears the terminal and reads the snapshot again (after a start that failed). */
  refill(agentId: string) {
    const entry = this.ensure(agentId);
    entry.term.reset();
    this.fill(entry);
  }

  focus(agentId: string) {
    this.entries.get(agentId)?.term.focus();
  }

  /** xterm's input element, for a dialog's initial focus. */
  inputOf(agentId: string): HTMLElement | null {
    return this.entries.get(agentId)?.term.textarea ?? null;
  }

  selectionOf(agentId: string): string {
    return this.entries.get(agentId)?.term.getSelection() ?? "";
  }

  /** Types `text` into the session the way a paste would. */
  paste(agentId: string, text: string) {
    this.entries.get(agentId)?.term.paste(text);
  }

  dispose() {
    this.disposed = true;
    this.unlisten?.();
    this.themeObserver.disconnect();
    for (const entry of this.entries.values()) {
      if (entry.frame !== undefined) cancelAnimationFrame(entry.frame);
      window.clearTimeout(entry.timer);
      entry.term.dispose();
      entry.host.remove();
    }
    this.entries.clear();
  }
}
