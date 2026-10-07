const RELATIVE = new Intl.RelativeTimeFormat(undefined, { numeric: "auto", style: "short" });
const FULL = new Intl.DateTimeFormat(undefined, { dateStyle: "full", timeStyle: "short" });

/** "just now", "5 min. ago", "3 hr. ago", "yesterday", "4 days ago", … */
export function formatAge(ms: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - ms) / 1000));
  if (seconds < 45) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return RELATIVE.format(-minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (hours < 24) return RELATIVE.format(-hours, "hour");
  const days = Math.round(hours / 24);
  if (days < 30) return RELATIVE.format(-days, "day");
  const months = Math.round(days / 30);
  if (months < 12) return RELATIVE.format(-months, "month");
  return RELATIVE.format(-Math.round(days / 365), "year");
}

export const formatFull = (ms: number) => FULL.format(new Date(ms));
