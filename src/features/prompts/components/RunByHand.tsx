import { ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { PressButton } from "@/shared";

/** How to start an agent yourself, until Foundry starts them. Collapsed by default. */
export function RunByHand() {
  return (
    <Collapsible className="group/run flex flex-col gap-1">
      <CollapsibleTrigger
        render={
          <PressButton
            variant="ghost"
            size="sm"
            className="-ml-2 w-fit gap-1.5 rounded-md px-2 text-[13px] font-medium text-muted-foreground"
          />
        }
      >
        <ChevronRight
          aria-hidden
          className="size-3.5 transition-transform duration-150 group-data-[open]/run:rotate-90 motion-reduce:transition-none"
        />
        Run it by hand
      </CollapsibleTrigger>
      <CollapsibleContent>
        <p className="rounded-lg bg-muted/60 px-3 py-2.5 text-[13px] leading-snug text-pretty ring-1 ring-foreground/5">
          Until agents start from Foundry, open a terminal in the project folder, run <code className="font-mono text-[12px]">claude</code>, and
          paste the launch message.
        </p>
      </CollapsibleContent>
    </Collapsible>
  );
}
