import { useEffect, useState, type CSSProperties } from "react";
import { readAccent, tintStyle } from "../lib/avatarTint";

/** One read per image source for the app's lifetime (every agent on the default avatar shares one). */
const cache = new Map<string, Promise<CSSProperties | undefined>>();

function tintOf(src: string): Promise<CSSProperties | undefined> {
  let tint = cache.get(src);
  if (!tint) {
    tint = readAccent(src).then((accent) => (accent ? tintStyle(accent) : undefined));
    cache.set(src, tint);
  }
  return tint;
}

/**
 * Each agent's card colours, derived from its avatar (`srcOf`, so a real avatar from
 * `agentAvatar` replaces the default as soon as it loads). Undefined while reading, or if the
 * image can't be read: the card is neutral then.
 */
export function useAvatarTints(
  agentIds: readonly string[],
  srcOf: (agentId: string) => string,
): (agentId: string) => CSSProperties | undefined {
  const [bySrc, setBySrc] = useState<Record<string, CSSProperties | undefined>>({});

  const sources = [...new Set(agentIds.map(srcOf))].join("\n");
  useEffect(() => {
    let cancelled = false;
    for (const src of sources ? sources.split("\n") : []) {
      void tintOf(src).then((tint) => {
        if (!cancelled) setBySrc((current) => (src in current ? current : { ...current, [src]: tint }));
      });
    }
    return () => {
      cancelled = true;
    };
  }, [sources]);

  return (agentId: string) => bySrc[srcOf(agentId)];
}
