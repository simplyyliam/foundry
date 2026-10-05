import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { SliceStatus } from "@/lib/bindings";
import { STATUS_LABEL } from "../lib/order";

const VARIANT = { queued: "outline", in_progress: "secondary", done: "default" } as const;

export function StatusChip({ status, className }: { status: SliceStatus; className?: string }) {
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
