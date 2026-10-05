import { useEffect, useState } from "react";
import { commands, type FolderLocations } from "@/lib/bindings";
import { call } from "./call";

/** Home and drive roots for the folder picker's shortcut chips; null until known (or if unavailable). */
export function useFolderLocations(): FolderLocations | null {
  const [locations, setLocations] = useState<FolderLocations | null>(null);

  useEffect(() => {
    let cancelled = false;
    call(() => commands.folderLocations()).then((outcome) => {
      // Without them the picker still works; it just has no shortcut chips.
      if (!cancelled && outcome.ok) setLocations(outcome.data);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return locations;
}
