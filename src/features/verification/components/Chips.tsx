import { CircleCheck, CircleDashed, CircleX, FileCheck2, FileX2, History } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { FindingSeverity, RequirementStatus, SliceStatus, VerificationState } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { STATUS_LABEL } from "../lib/verification";

const CHIP = "h-5 px-2 text-[11px]";
const STATUS_VARIANT = { queued: "outline", in_progress: "secondary", done: "default" } as const;

/** A slice's status, styled like the Blueprint's chip (that one can't be imported). */
export function SliceStatusChip({ status }: { status: SliceStatus }) {
  return (
    <Badge
      variant={STATUS_VARIANT[status]}
      className={cn(CHIP, status === "queued" && "border-slice/50 bg-background/60 text-slice-foreground")}
    >
      {STATUS_LABEL[status]}
    </Badge>
  );
}

export function ReportChip({ present }: { present: boolean }) {
  return present ? (
    <Badge variant="secondary" className={CHIP}>
      <FileCheck2 aria-hidden data-icon="inline-start" />
      Report
    </Badge>
  ) : (
    <Badge variant="outline" className={cn(CHIP, "text-muted-foreground")}>
      <FileX2 aria-hidden data-icon="inline-start" />
      No report
    </Badge>
  );
}

const VERIFICATION = {
  passed: { label: "Passed", Icon: CircleCheck, className: "border-status-running/40 text-foreground [&>svg]:text-status-running" },
  failed: { label: "Failed", Icon: CircleX, className: "border-transparent bg-destructive/10 text-destructive" },
  none: { label: "Not verified", Icon: CircleDashed, className: "text-muted-foreground" },
  stale: { label: "Stale", Icon: History, className: "border-warning/50 text-foreground [&>svg]:text-warning" },
} satisfies Record<VerificationState, unknown>;

/** passed / failed / not verified / stale: an icon and a label, never colour alone. */
export function VerificationChip({ state }: { state: VerificationState }) {
  const { label, Icon, className } = VERIFICATION[state];
  return (
    <Badge variant="outline" className={cn(CHIP, className)}>
      <Icon aria-hidden data-icon="inline-start" />
      {label}
    </Badge>
  );
}

const REQUIREMENT = {
  met: { label: "met", variant: "default" },
  partly: { label: "partly", variant: "secondary" },
  not_met: { label: "not met", variant: "destructive" },
} as const;

export function RequirementStatusChip({ status }: { status: RequirementStatus }) {
  const { label, variant } = REQUIREMENT[status];
  return (
    <Badge variant={variant} className={CHIP}>
      {label}
    </Badge>
  );
}

const SEVERITY = {
  danger: { label: "Danger", className: "border-transparent bg-destructive/10 text-destructive" },
  warn: { label: "Warning", className: "border-warning/50 text-foreground" },
  info: { label: "Info", className: "text-muted-foreground" },
} satisfies Record<FindingSeverity, unknown>;

export function SeverityChip({ severity }: { severity: FindingSeverity }) {
  const { label, className } = SEVERITY[severity];
  return (
    <Badge variant="outline" className={cn(CHIP, className)}>
      {label}
    </Badge>
  );
}
