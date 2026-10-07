import type { Plan, PromptFile, SliceStatus, SliceView } from "@/lib/bindings";

/** Slices by layer, then name (ids break ties so the order never flickers), like the Blueprint. */
export function orderSlices(slices: readonly SliceView[]): SliceView[] {
  return [...slices].sort((a, b) => a.layer - b.layer || a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
}

/** The hand-off first, then task prompts in the backend's order (by path). */
export function orderPrompts(files: readonly PromptFile[]): PromptFile[] {
  return [...files.filter((file) => file.kind === "handoff"), ...files.filter((file) => file.kind === "task")];
}

export const STATUS_LABEL: Record<SliceStatus, string> = {
  queued: "queued",
  in_progress: "in progress",
  done: "done",
};

const DEPENDENCIES_PREFIX = "these dependencies aren't done yet: ";
const DEPENDENCIES_SUFFIX = " (generate with force to start anyway)";

/**
 * The dependency names inside a `dependencies_not_done` message, which only carries them as
 * text. A fresh plan's unfinished dependencies are used when they join to exactly that text
 * (so a name containing ", " survives); otherwise the text is split on ", ". Null when the
 * message has another shape: show it as-is then.
 */
export function unfinishedFromMessage(message: string, plan: Plan | null, sliceId: string): string[] | null {
  if (!message.startsWith(DEPENDENCIES_PREFIX) || !message.endsWith(DEPENDENCIES_SUFFIX)) return null;
  const inner = message.slice(DEPENDENCIES_PREFIX.length, message.length - DEPENDENCIES_SUFFIX.length);
  if (!inner) return null;
  const slice = plan?.slices.find((candidate) => candidate.id === sliceId);
  if (plan && slice) {
    const names = slice.dependsOn
      .map((id) => plan.slices.find((candidate) => candidate.id === id))
      .filter((dependency) => dependency !== undefined && dependency.status !== "done")
      .map((dependency) => dependency!.name);
    if (names.join(", ") === inner) return names;
  }
  return inner.split(", ");
}

export const decisionText = (names: readonly string[]) => `Started with unfinished dependencies: ${names.join(", ")}`;

export function formatSize(bytes: number): string {
  if (bytes < 1000) return `${bytes} B`;
  if (bytes < 1_000_000) return `${(bytes / 1000).toFixed(bytes < 10_000 ? 1 : 0)} KB`;
  return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

const TIME = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const DAY = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const DAY_YEAR = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" });
const FULL = new Intl.DateTimeFormat(undefined, { dateStyle: "full", timeStyle: "medium" });

/** "Today, 14:05", "Yesterday, 09:12", "Oct 3, 18:40", or with the year when it isn't this one. */
export function formatModified(ms: number, now = new Date()): string {
  const date = new Date(ms);
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const time = TIME.format(date);
  if (ms >= startOfToday) return `Today, ${time}`;
  if (ms >= startOfToday - 86_400_000) return `Yesterday, ${time}`;
  const day = date.getFullYear() === now.getFullYear() ? DAY.format(date) : DAY_YEAR.format(date);
  return `${day}, ${time}`;
}

export const formatModifiedFull = (ms: number) => FULL.format(new Date(ms));
