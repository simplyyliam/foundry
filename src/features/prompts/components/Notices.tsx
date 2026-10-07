import { ArrowUpRight, Info, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { buttonVariants } from "@/components/ui/button";
import { PROJECT_SECTIONS, projectSectionPath } from "@/features/projects";
import type { AppError } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { usePressScale } from "@/shared";

/** A backend message, as-is, with an optional action beside it. */
export function ErrorLine({ message, children, className }: { message: string; children?: ReactNode; className?: string }) {
  return (
    <div role="alert" className={cn("flex items-start gap-2 rounded-lg bg-destructive/8 py-2 pr-1.5 pl-3 text-[12.5px]", className)}>
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 py-px leading-snug">{message}</p>
      {children}
    </div>
  );
}

/** Things the result mentions that didn't refuse it (e.g. unlabeled Bakery images), as-is. */
export function WarningNotice({ warnings }: { warnings: readonly string[] }) {
  if (warnings.length === 0) return null;
  return (
    <div role="status" className="flex items-start gap-2 rounded-lg bg-muted/70 px-3 py-2 text-[12.5px] ring-1 ring-foreground/5">
      <Info aria-hidden className="mt-px size-3.5 shrink-0 text-muted-foreground" />
      <ul className="flex min-w-0 flex-1 flex-col gap-1 leading-snug">
        {warnings.map((warning, index) => (
          <li key={index}>{warning}</li>
        ))}
      </ul>
    </div>
  );
}

/** Where each gate is fixed: the backend's message plus a link to that page. */
const GATE_SECTION: Partial<Record<AppError["code"], { section: string; label: string }>> = {
  plan_not_confirmed: { section: "requirements", label: "Requirements" },
  agents_md_not_ready: { section: "setup", label: "Setup" },
  team_not_confirmed: { section: "setup", label: "Setup" },
};

/** A refused generation: the message as-is, with a link to the page that fixes it, if any. */
export function GateError({ projectId, error }: { projectId: string; error: AppError }) {
  const gate = GATE_SECTION[error.code];
  return (
    <ErrorLine message={error.message}>
      {gate && <SectionLink projectId={projectId} sectionId={gate.section} label={gate.label} />}
    </ErrorLine>
  );
}

/** A small link to another page of this project, e.g. "Setup ↗". */
export function SectionLink({ projectId, sectionId, label }: { projectId: string; sectionId: string; label: string }) {
  const ref = usePressScale<HTMLAnchorElement>();
  const section = PROJECT_SECTIONS.find((candidate) => candidate.id === sectionId);
  if (!section) return null;
  return (
    <Link
      ref={ref}
      to={projectSectionPath(projectId, section)}
      className={cn(buttonVariants({ variant: "outline", size: "xs" }), "shrink-0 rounded-md bg-background")}
    >
      {label}
      <ArrowUpRight aria-hidden data-icon="inline-end" />
    </Link>
  );
}
