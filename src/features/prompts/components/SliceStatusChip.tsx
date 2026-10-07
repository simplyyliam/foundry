import { Badge } from "@/components/ui/badge";
import type { SliceStatus } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { STATUS_LABEL } from "../lib/prompts";

const VARIANT = { queued: "outline", in_progress: "secondary", done: "default" } as const;

/** A slice's status, styled like the Blueprint's chip (that one can't be imported). */
export function SliceStatusChip({ status, className }: { status: SliceStatus; className?: string }) {
  return (
    <Badge
      variant={VARIANT[status]}
      className={cn(
        "h-4 shrink-0 px-1.5 text-[10px]",
        status === "queued" && "border-slice/50 bg-background/60 text-slice-foreground",
        className,
      )}
    >
      {STATUS_LABEL[status]}
    </Badge>
  );
}
