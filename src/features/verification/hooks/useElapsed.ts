import { useEffect, useState } from "react";

/** Counts up from the last progress event's `elapsedMs`, once a second, until `running` ends. */
export function useElapsed(elapsedMs: number, receivedAt: number, running: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [running, receivedAt]);
  return elapsedMs + Math.max(0, now - receivedAt);
}
