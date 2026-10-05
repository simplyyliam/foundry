import { useEffect, useState } from "react";
import { commands, type AppInfo } from "@/lib/bindings";
import { call, type Outcome } from "./call";

/** `appInfo` once per mount; null while loading. */
export function useAppInfo(): Outcome<AppInfo> | null {
  const [info, setInfo] = useState<Outcome<AppInfo> | null>(null);

  useEffect(() => {
    let cancelled = false;
    call(() => commands.appInfo()).then((outcome) => {
      if (!cancelled) setInfo(outcome);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return info;
}
