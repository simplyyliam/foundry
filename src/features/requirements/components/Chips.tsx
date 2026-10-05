import { Check } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { RequirementKind, RequirementState } from "@/lib/bindings";

const CHIP = "h-5 px-2 text-[11px]";

export function KindChip({ kind }: { kind: RequirementKind }) {
  return (
    <Badge variant="secondary" className={CHIP}>
      {kind}
    </Badge>
  );
}

export function StateChip({ state }: { state: RequirementState }) {
  return state === "confirmed" ? (
    <Badge variant="default" className={CHIP}>
      <Check aria-hidden data-icon="inline-start" />
      confirmed
    </Badge>
  ) : (
    <Badge variant="outline" className={CHIP}>
      proposed
    </Badge>
  );
}

export function UnsourcedBadge() {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          // Focusable, so the explanation is reachable from the keyboard too.
          <Badge variant="destructive" tabIndex={0} className={`${CHIP} cursor-help outline-none`}>
            unsourced
          </Badge>
        }
      />
      <TooltipContent>Couldn't be matched to your brain dump</TooltipContent>
    </Tooltip>
  );
}
