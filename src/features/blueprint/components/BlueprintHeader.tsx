import { Link } from "react-router-dom";
import { ArrowUpRight, Plus } from "lucide-react";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { Plan } from "@/lib/bindings";
import { PressButton, usePressScale } from "@/shared";
import { plural } from "../lib/order";

interface BlueprintHeaderProps {
  plan: Plan;
  requirementsHref: string;
  onAddSlice: () => void;
}

/** "N requirements · M confirmed", a link to the Requirements page, and Add slice. */
export function BlueprintHeader({ plan, requirementsHref, onAddSlice }: BlueprintHeaderProps) {
  const linkRef = usePressScale<HTMLAnchorElement>();
  const confirmed = plan.requirements.filter((requirement) => requirement.state === "confirmed").length;

  return (
    <div className="flex h-11 shrink-0 items-center gap-3 border-b border-border/60 px-4">
      <p className="text-[13px] text-muted-foreground tabular-nums">
        <span className="font-medium text-foreground">{plural(plan.requirements.length, "requirement")}</span>
        {" · "}
        {confirmed} confirmed
      </p>
      <Link
        ref={linkRef}
        to={requirementsHref}
        className={cn(buttonVariants({ variant: "ghost", size: "xs" }), "text-muted-foreground")}
      >
        Requirements
        <ArrowUpRight aria-hidden data-icon="inline-end" />
      </Link>
      <PressButton variant="secondary" size="sm" className="ml-auto" onClick={onAddSlice}>
        <Plus aria-hidden data-icon="inline-start" />
        Add slice
      </PressButton>
    </div>
  );
}
