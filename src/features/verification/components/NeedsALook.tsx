import { ChevronRight, CircleAlert, CircleSlash, TriangleAlert } from "lucide-react";
import type { AttentionItem, SliceView, VerifySkip } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import { bySeverity, skipText } from "../lib/verification";

interface NeedsALookProps {
  items: readonly AttentionItem[];
  /** Slices whose wanted verification was skipped or can't run, in plan order. */
  skips: readonly VerifySkip[];
  slices: readonly SliceView[];
  onOpenSlice: (sliceId: string) => void;
}

/**
 * The digest's attention items, most severe first, then the skipped slices. Renders nothing
 * when there are none.
 */
export function NeedsALook({ items, skips, slices, onOpenSlice }: NeedsALookProps) {
  if (items.length === 0 && skips.length === 0) return null;
  return (
    <section aria-labelledby="needs-a-look-title" className="flex flex-col gap-1.5 rounded-2xl bg-muted px-2 pt-3 pb-2">
      <h2 id="needs-a-look-title" className="px-2 text-[13px] leading-snug font-medium">
        Needs a look
      </h2>
      <ul className="flex flex-col">
        {bySeverity(items).map((item, index) => {
          const slice = item.sliceId ? slices.find((candidate) => candidate.id === item.sliceId) : undefined;
          return (
            <li key={`${item.kind}-${item.sliceId ?? ""}-${item.eventId ?? ""}-${index}`}>
              <AttentionRow item={item} sliceName={slice?.name ?? item.sliceId} onOpen={slice ? () => onOpenSlice(slice.id) : undefined} />
            </li>
          );
        })}
        {skips.map((skip) => {
          const slice = slices.find((candidate) => candidate.id === skip.sliceId);
          return (
            <li key={`skip-${skip.sliceId}`}>
              <AttentionRow
                skip={skip}
                sliceName={slice?.name ?? skip.sliceId}
                onOpen={slice ? () => onOpenSlice(slice.id) : undefined}
              />
            </li>
          );
        })}
      </ul>
    </section>
  );
}

type AttentionRowProps = ({ item: AttentionItem; skip?: never } | { skip: VerifySkip; item?: never }) & {
  sliceName: string | null;
  onOpen?: () => void;
};

/** An attention item, or a skipped verification (its reason in plain words). */
function AttentionRow({ item, skip, sliceName, onOpen }: AttentionRowProps) {
  const danger = item?.severity === "danger";
  const Icon = skip ? CircleSlash : danger ? TriangleAlert : CircleAlert;
  const content = (
    <>
      <Icon aria-hidden className={cn("mt-0.5 size-3.5 shrink-0", danger ? "text-destructive" : "text-warning")} />
      <span className="sr-only">{skip ? "Not verified:" : danger ? "Danger:" : "Warning:"}</span>
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-[13px] leading-snug text-pretty whitespace-normal">{skip ? skipText(skip) : item.message}</span>
        {sliceName && <span className="truncate text-[12px] text-muted-foreground">{sliceName}</span>}
      </span>
    </>
  );
  if (!onOpen) return <div className="flex items-start gap-2.5 px-2 py-1.5">{content}</div>;
  return (
    <PressButton
      variant="ghost"
      pressScale={0.99}
      onClick={onOpen}
      className="group h-auto w-full items-start justify-start gap-2.5 rounded-lg px-2 py-1.5 text-left font-normal hover:bg-background/70"
    >
      {content}
      <ChevronRight aria-hidden className="mt-0.5 size-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100" />
    </PressButton>
  );
}
