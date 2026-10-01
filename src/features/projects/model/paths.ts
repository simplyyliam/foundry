/** Path helpers that work on both Windows (`C:\a\b`) and POSIX (`/a/b`) paths. */

export function basename(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.at(-1) ?? path;
}

/** Comparable form: Windows paths are case-insensitive. */
export function pathKey(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  return /^[a-z]:/i.test(trimmed) || trimmed.includes("\\")
    ? trimmed.replace(/\//g, "\\").toLowerCase()
    : trimmed;
}
