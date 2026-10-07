import { useCallback, useEffect, useState } from "react";

export type CopyState = "idle" | "copied" | "failed";

/** Copies text to the clipboard; the state shows "copied" for a moment. */
export function useCopy(): [CopyState, (text: string) => Promise<boolean>] {
  const [state, setState] = useState<CopyState>("idle");

  useEffect(() => {
    if (state !== "copied") return;
    const timer = setTimeout(() => setState("idle"), 1500);
    return () => clearTimeout(timer);
  }, [state]);

  const copy = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setState("copied");
      return true;
    } catch (error) {
      // The webview can refuse clipboard access; the text stays selectable on screen.
      console.warn("Couldn't copy to the clipboard", error);
      setState("failed");
      return false;
    }
  }, []);

  return [state, copy];
}
