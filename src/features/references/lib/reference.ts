import { convertFileSrc } from "@tauri-apps/api/core";
import type { BakeryItem } from "@/lib/bindings";

/** The image through the asset protocol: `<dir>/<file>` (the backend scopes `docs/bakery/`). */
export function imageSrc(dir: string, file: string): string {
  const separator = dir.includes("\\") ? "\\" : "/";
  return convertFileSrc(`${dir.replace(/[\\/]+$/, "")}${separator}${file}`);
}

/** Agents ignore an image without a label or a Take note (the backend's `unlabeledCount`). */
export const needsNote = (item: BakeryItem) => item.label.trim() === "" || item.take.trim() === "";

export const newestFirst = (items: readonly BakeryItem[]) => [...items].reverse();

export const parseList = (text: string) =>
  text
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);

export const NEEDS_NOTE_TOOLTIP = "Agents ignore references that have no label and no note.";
