import { Check, LoaderCircle } from "lucide-react";
import type { SaveState } from "../hooks/useSaveState";

/** "Saving…" while a save runs, "Saved" once the last one landed. */
export function SaveStatus({ state }: { state: SaveState }) {
  if (state === "idle") return null;
  return (
    <span role="status" className="flex items-center gap-1 text-[12px] text-muted-foreground animate-in fade-in duration-200 motion-reduce:animate-none">
      {state === "saving" ? <LoaderCircle aria-hidden className="size-3 animate-spin" /> : <Check aria-hidden className="size-3" />}
      {state === "saving" ? "Saving…" : "Saved"}
    </span>
  );
}
