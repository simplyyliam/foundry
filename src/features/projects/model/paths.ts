/** Path helpers that work on both Windows (`C:\a\b`) and POSIX (`/a/b`) paths. */

const separatorOf = (path: string) => (path.includes("\\") ? "\\" : "/");

export function joinPath(parent: string, name: string): string {
  const separator = separatorOf(parent);
  return parent.endsWith(separator) ? `${parent}${name}` : `${parent}${separator}${name}`;
}

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

const RESERVED = /^(con|prn|aux|nul|com\d|lpt\d)(\..*)?$/i;

/** Why `name` can't be a folder name on any desktop OS, or null if it can. */
export function folderNameError(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "Enter a name.";
  if (trimmed !== name) return "Remove the spaces at the start or end.";
  if (/[<>:"/\\|?*]/.test(name)) return 'Names can\'t contain < > : " / \\ | ? *';
  // Control characters aren't valid in folder names.
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f]/.test(name)) return "Names can't contain control characters.";
  if (name.endsWith(".")) return "Names can't end with a period.";
  if (name === "." || name === "..") return "Choose a different name.";
  if (RESERVED.test(name)) return `“${name}” is reserved by Windows.`;
  if (name.length > 100) return "Use 100 characters or fewer.";
  return null;
}
