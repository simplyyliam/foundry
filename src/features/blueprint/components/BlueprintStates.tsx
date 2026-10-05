import { Plus } from "lucide-react";
import { PressButton } from "@/shared";

/** The first load failed: the backend's message, as-is. */
export function LoadFailed({ message }: { message: string }) {
  return (
    <div className="flex h-full w-full items-center justify-center bg-background px-8">
      <p role="alert" className="max-w-md text-center text-sm text-muted-foreground">
        {message}
      </p>
    </div>
  );
}

/** Over the canvas when the plan has no slices yet. */
export function NoSlices({ onAddSlice }: { onAddSlice: () => void }) {
  return (
    <div className="pointer-events-auto flex max-w-sm flex-col items-center gap-3 rounded-3xl bg-popover/95 px-6 py-5 text-center shadow-lg ring-1 ring-foreground/5">
      <div className="flex flex-col gap-1">
        <p className="text-sm font-medium">This plan has no slices yet</p>
        <p className="text-[13px] text-pretty text-muted-foreground">
          Add one by hand, or generate a roadmap from the Roadmap panel to fill the blueprint.
        </p>
      </div>
      <PressButton size="sm" onClick={onAddSlice}>
        <Plus aria-hidden data-icon="inline-start" />
        Add slice
      </PressButton>
    </div>
  );
}
