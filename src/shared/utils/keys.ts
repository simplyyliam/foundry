const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** A shortcut label for this platform: `shortcut("B")` → "Ctrl+B" (Windows) or "⌘B" (macOS). */
export function shortcut(key: string, { shift = false }: { shift?: boolean } = {}): string {
  return isMac ? `${shift ? "⇧" : ""}⌘${key}` : `Ctrl+${shift ? "Shift+" : ""}${key}`;
}
