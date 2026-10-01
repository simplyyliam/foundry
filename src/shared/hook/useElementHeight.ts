import { useLayoutEffect, useRef, useState } from "react";

/**
 * Tracks an element's rendered height with a ResizeObserver. Pair it with a
 * motion wrapper (`animate={{ height }}`) to animate a container smoothly as its
 * content changes, without animating layout on the content itself.
 */
export function useElementHeight<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [height, setHeight] = useState<number | "auto">("auto");

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      setHeight(entry.borderBoxSize[0]?.blockSize ?? element.offsetHeight);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return [ref, height] as const;
}
