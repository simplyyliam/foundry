import { useEffect, useState } from "react";
import { commands } from "@/lib/bindings";

/**
 * The default projects folder from settings, read fresh on every mount so a change in the
 * Settings dialog applies the next time the add-project dialog opens.
 * `undefined` while loading; `null` when none is set or settings couldn't be read.
 */
export function useDefaultProjectsDir(): string | null | undefined {
  const [dir, setDir] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    commands
      .getSettings()
      .then((result) => {
        if (cancelled) return;
        if (result.status === "ok") setDir(result.data.defaultProjectsDir);
        else {
          console.error("getSettings failed:", result.error.message);
          setDir(null);
        }
      })
      .catch((error) => {
        // Outside Tauri: fall back to the folder picked last time.
        console.error("getSettings failed:", error);
        if (!cancelled) setDir(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return dir;
}
