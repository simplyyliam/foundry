import { LoaderCircle } from "lucide-react";
import type { ReactNode } from "react";
import { PressButton } from "@/shared";
import type { EventStream } from "../hooks/useAgentDetail";
import { ErrorLine } from "./ErrorLine";

export const EMPTY_TEXT = "Nothing yet. Events appear when this agent runs.";

/** A small grey heading pill, like the design's "Changes" and "Danger". */
export function SectionPill({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex h-6 w-fit items-center rounded-[10px] bg-pill px-2 text-[11.5px] font-medium">{children}</span>
  );
}

/** "Load older" with its spinner and error; nothing once the oldest event is loaded. */
export function LoadOlder({ stream, onLoad }: { stream: EventStream; onLoad: () => void }) {
  if (stream.exhausted) return null;
  return (
    <div className="flex flex-col items-center gap-2 py-1">
      {stream.olderError && <ErrorLine message={stream.olderError} className="w-full" />}
      <PressButton
        type="button"
        size="xs"
        variant="ghost"
        className="rounded-[10px] text-[11.5px] text-muted-foreground"
        disabled={stream.loadingOlder}
        onClick={onLoad}
      >
        {stream.loadingOlder && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
        Load older
      </PressButton>
    </div>
  );
}

/**
 * What a tab shows instead of its items: loading, the first page's error, or the empty text.
 * Null when the tab has items.
 */
export function StreamState({
  stream,
  empty,
  onRetry,
  onLoadOlder,
}: {
  stream: EventStream;
  empty: boolean;
  onRetry: () => void;
  onLoadOlder: () => void;
}) {
  if (stream.status === "loading") {
    return (
      <div className="flex justify-center py-10">
        <LoaderCircle aria-label="Loading events" className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (stream.status === "error") {
    return (
      <ErrorLine message={stream.error ?? ""}>
        <PressButton size="xs" variant="outline" className="rounded-md" onClick={onRetry}>
          Try again
        </PressButton>
      </ErrorLine>
    );
  }
  if (!empty) return null;
  return (
    <div className="flex flex-col items-center gap-2 px-4 py-10 text-center">
      <p className="text-[12.5px] text-muted-foreground">{EMPTY_TEXT}</p>
      {/* Only older events left to look through (the first pages had none for this tab). */}
      {!stream.exhausted && <LoadOlder stream={stream} onLoad={onLoadOlder} />}
    </div>
  );
}
