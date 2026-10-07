import type { ITheme } from "@xterm/xterm";

/** The app ships no monospace font, so the terminal uses the system's. */
export const TERMINAL_FONT =
  'ui-monospace, "Cascadia Mono", "Cascadia Code", Consolas, "SF Mono", Menlo, "DejaVu Sans Mono", monospace';

/*
 * ANSI palettes. These are terminal data (what an app's "red" means), not UI colours, so they
 * can't come from the theme tokens. The light one is darkened to stay readable on the card tints.
 */
const LIGHT_ANSI: ITheme = {
  black: "#1f1f1f",
  red: "#c4314b",
  green: "#2f7d32",
  yellow: "#856a00",
  blue: "#1f5fbf",
  magenta: "#8e3fa8",
  cyan: "#0f7484",
  white: "#6b6b6b",
  brightBlack: "#5c5c5c",
  brightRed: "#d6455d",
  brightGreen: "#388e3c",
  brightYellow: "#967800",
  brightBlue: "#2b6fd6",
  brightMagenta: "#a24dbd",
  brightCyan: "#11899a",
  brightWhite: "#3b3b3b",
};

const DARK_ANSI: ITheme = {
  black: "#000000",
  red: "#cd3131",
  green: "#0dbc79",
  yellow: "#e5e510",
  blue: "#2472c8",
  magenta: "#bc3fbc",
  cyan: "#11a8cd",
  white: "#e5e5e5",
  brightBlack: "#666666",
  brightRed: "#f14c4c",
  brightGreen: "#23d18b",
  brightYellow: "#f5f543",
  brightBlue: "#3b8eea",
  brightMagenta: "#d670d6",
  brightCyan: "#29b8db",
  brightWhite: "#e5e5e5",
};

let probe: CanvasRenderingContext2D | null | undefined;

const hex = (value: number) => value.toString(16).padStart(2, "0");

/**
 * A theme token as `#rrggbbaa`. Tokens are oklch, which xterm can't parse, so a canvas
 * converts them. Undefined if the token is missing or the canvas isn't available.
 */
function tokenColor(name: string, alpha = 1): string | undefined {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  if (!raw) return undefined;
  probe ??= document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  if (!probe) return undefined;
  probe.clearRect(0, 0, 1, 1);
  probe.fillStyle = "#000";
  probe.fillStyle = raw;
  probe.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data;
  return `#${hex(r)}${hex(g)}${hex(b)}${hex(Math.round(a * alpha))}`;
}

/** The terminal colours for the app's current theme (the `.dark` class on `<html>`). */
export function terminalTheme(): ITheme {
  const dark = document.documentElement.classList.contains("dark");
  const foreground = tokenColor("--foreground") ?? (dark ? "#fafafa" : "#1a1a1a");
  return {
    ...(dark ? DARK_ANSI : LIGHT_ANSI),
    // Transparent over the card tint (needs `allowTransparency`).
    background: "#00000000",
    foreground,
    cursor: foreground,
    cursorAccent: tokenColor("--background") ?? (dark ? "#1a1a1a" : "#ffffff"),
    selectionBackground: tokenColor("--ring", 0.4) ?? (dark ? "#ffffff40" : "#00000030"),
  };
}
