import type { CSSProperties } from "react";

/** A colour in OKLCH: lightness 0–1, chroma, hue in degrees. */
export interface Oklch {
  l: number;
  c: number;
  h: number;
}

/** Below this chroma an avatar counts as grey: neutral card, and the name in normal text. */
const CHROMATIC = 0.035;
/** The chroma at which a card reaches its full tint (the mockup's #1E718F is about 0.09). */
const FULL_TINT_CHROMA = 0.09;
/** How much darker (light theme) or lighter (dark theme) hover and selected are. */
const HOVER_STEP = 0.02;
const SELECTED_STEP = 0.045;

const toLinear = (value: number) => {
  const v = value / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
};

/** sRGB (0–255) to OKLab. */
function toOklab(r: number, g: number, b: number): [number, number, number] {
  const [lr, lg, lb] = [toLinear(r), toLinear(g), toLinear(b)];
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

const SAMPLE = 48;

/**
 * The avatar's accent: the colour of its background. Avatars are a figure on a backdrop (the
 * mockup's Mark is on #1E718F), so this averages, in OKLab, the pixels of a ring near the
 * edge, top half only (the figure's body usually fills the bottom), skipping transparent,
 * near-black (outlines, hair) and near-white (highlights) pixels. Null if the image can't be
 * read.
 */
export function readAccent(src: string): Promise<Oklch | null> {
  return new Promise((resolve) => {
    const image = new Image();
    image.decoding = "async";
    image.onerror = () => resolve(null);
    image.onload = () => {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = canvas.height = SAMPLE;
        const context = canvas.getContext("2d", { willReadFrequently: true });
        if (!context) return resolve(null);
        context.drawImage(image, 0, 0, SAMPLE, SAMPLE);
        const { data } = context.getImageData(0, 0, SAMPLE, SAMPLE);
        const center = (SAMPLE - 1) / 2;
        let sumL = 0;
        let sumA = 0;
        let sumB = 0;
        let count = 0;
        for (let y = 0; y < SAMPLE / 2; y++) {
          for (let x = 0; x < SAMPLE; x++) {
            const radius = Math.hypot(x - center, y - center) / center;
            if (radius < 0.7 || radius > 0.98) continue;
            const i = (y * SAMPLE + x) * 4;
            if (data[i + 3] < 200) continue;
            const [l, a, b] = toOklab(data[i], data[i + 1], data[i + 2]);
            if (l < 0.18 || l > 0.97) continue;
            sumL += l;
            sumA += a;
            sumB += b;
            count++;
          }
        }
        if (count === 0) return resolve(null);
        const a = sumA / count;
        const b = sumB / count;
        resolve({ l: sumL / count, c: Math.hypot(a, b), h: ((Math.atan2(b, a) * 180) / Math.PI + 360) % 360 });
      } catch {
        resolve(null);
      }
    };
    image.src = src;
  });
}

const css = ({ l, c, h }: Oklch) => `oklch(${l.toFixed(3)} ${c.toFixed(3)} ${h.toFixed(1)})`;

/**
 * The card colours derived from an accent, as CSS variables for `agent-tint.css`, for both
 * themes. Light: the accent lightened and muted for the background (#1E718F → about #B5D0DD),
 * the accent itself for the name, a dark muted shade for the title. Dark: a deep muted shade
 * for the background and a light one for the name. Hover and selected are the background a
 * step darker (light) or lighter (dark). A grey accent gives a neutral card close to #F0F0F3,
 * and leaves the name and title on the theme's text colours.
 */
export function tintStyle(accent: Oklch): CSSProperties {
  const { c, h } = accent;
  const tint = Math.min(c / FULL_TINT_CHROMA, 1);
  const chromatic = c >= CHROMATIC;

  const light = { l: 0.955 - 0.115 * tint, c: Math.min(c * 0.35, 0.045), h };
  const dark = { l: 0.225 + 0.05 * tint, c: Math.min(c * 0.45, 0.05), h };
  const shift = (base: Oklch, by: number) => css({ ...base, l: base.l + by });

  const vars: Record<string, string> = {
    "--avatar-bg-light": css(light),
    "--avatar-bg-hover-light": shift(light, -HOVER_STEP),
    "--avatar-bg-selected-light": shift(light, -SELECTED_STEP),
    "--avatar-bg-dark": css(dark),
    "--avatar-bg-hover-dark": shift(dark, HOVER_STEP),
    "--avatar-bg-selected-dark": shift(dark, SELECTED_STEP),
  };
  if (chromatic) {
    vars["--avatar-name-light"] = css({ l: Math.min(accent.l, 0.55), c, h });
    vars["--avatar-title-light"] = css({ l: 0.43, c: Math.min(c * 0.25, 0.03), h });
    vars["--avatar-name-dark"] = css({ l: Math.max(accent.l, 0.82), c: Math.min(c, 0.1), h });
    vars["--avatar-title-dark"] = css({ l: 0.76, c: Math.min(c * 0.3, 0.035), h });
  }
  return vars as CSSProperties;
}
