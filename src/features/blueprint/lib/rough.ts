import rough from "roughjs";
import type { Options } from "roughjs/bin/core";

/**
 * Hand-drawn outlines that hold still: one shared generator (no DOM), and every shape gets a
 * seed derived from its item's id. Same id + same geometry → the same path on every render,
 * so outlines don't jitter on re-render or while panning (React Flow pans with a transform).
 */
const generator = rough.generator();

/** FNV-1a over the id, mapped into rough.js's seed range (0 would mean "random"). */
export function seedFor(id: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < id.length; index++) {
    hash ^= id.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return ((hash >>> 0) % 2147483646) + 1;
}

/** One SVG path of a drawn shape: an outline stroke, or the fill underneath it. */
export interface RoughPath {
  d: string;
  role: "stroke" | "fill";
}

function toPaths(drawable: ReturnType<typeof generator.rectangle>): RoughPath[] {
  return drawable.sets.map((set) => ({
    d: generator.opsToPath(set, 2),
    // "fillSketch" (hachure) is drawn with strokes in the fill colour; we only use solid fills.
    role: set.type === "path" ? "stroke" : "fill",
  }));
}

// Colours are applied with classes on the paths (theme tokens), so these only shape the stroke.
const BASE: Options = { stroke: "currentColor", strokeWidth: 1.4, roughness: 1.1, bowing: 0.8 };

export function roughRect(id: string, width: number, height: number, options: Options = {}): RoughPath[] {
  const inset = 2; // keep the wobble inside the box
  return toPaths(
    generator.rectangle(inset, inset, Math.max(width - inset * 2, 1), Math.max(height - inset * 2, 1), {
      ...BASE,
      seed: seedFor(id),
      ...options,
    }),
  );
}

export function roughPath(id: string, d: string, options: Options = {}): RoughPath[] {
  return toPaths(generator.path(d, { ...BASE, seed: seedFor(id), ...options }));
}

export function roughLines(id: string, points: [number, number][], options: Options = {}): RoughPath[] {
  return toPaths(generator.linearPath(points, { ...BASE, seed: seedFor(id), ...options }));
}
