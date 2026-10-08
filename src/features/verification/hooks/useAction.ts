import { useCallback, useState } from "react";
import type { AppError } from "@/lib/bindings";
import type { Outcome } from "./call";

/** Runs one action at a time for a control, keeping its failure (as-is) to show beside it. */
export function useAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const run = useCallback(async <T,>(action: () => Promise<Outcome<T>>): Promise<Outcome<T>> => {
    setBusy(true);
    setError(null);
    const outcome = await action();
    setBusy(false);
    if (!outcome.ok) setError(outcome.error);
    return outcome;
  }, []);

  const clear = useCallback(() => setError(null), []);
  return { busy, error, run, clear, setError };
}
