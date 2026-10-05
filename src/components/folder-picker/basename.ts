/** Last segment of a Windows or POSIX path ("C:\\code\\app" → "app"). */
export function basename(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.at(-1) ?? path;
}
