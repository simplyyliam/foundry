import { useCallback, useRef, useState } from "react";
import type { Outcome } from "./call";

export type SaveState = "idle" | "saving" | "saved";

/** Tracks a sheet's saves for its "Saving…" / "Saved" line. Errors show beside each field. */
export function useSaveState() {
  const [state, setState] = useState<SaveState>("idle");
  const running = useRef(0);

  const track = useCallback(async <T,>(save: Promise<Outcome<T>>): Promise<Outcome<T>> => {
    running.current += 1;
    setState("saving");
    const outcome = await save;
    running.current -= 1;
    if (running.current === 0) setState(outcome.ok ? "saved" : "idle");
    return outcome;
  }, []);

  return { state, track };
}
