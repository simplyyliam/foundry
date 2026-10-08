import { ArrowUpRight, TriangleAlert, X } from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { buttonVariants } from "@/components/ui/button";
import { PROJECT_SECTIONS, projectSectionPath } from "@/features/projects";
import { cn } from "@/lib/utils";
import { PressButton, usePressScale } from "@/shared";

/** A backend message, as-is, with optional actions beside it. */
export function ErrorLine({ message, children, className }: { message: string; children?: ReactNode; className?: string }) {
  return (
    <div role="alert" className={cn("flex items-start gap-2 rounded-lg bg-destructive/8 py-2 pr-1.5 pl-3 text-[12.5px]", className)}>
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 py-px leading-snug break-words">{message}</p>
      {children}
    </div>
  );
}

export function DismissButton({ onClick, label = "Dismiss" }: { onClick: () => void; label?: string }) {
  return (
    <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label={label} onClick={onClick} className="shrink-0 rounded-full">
      <X aria-hidden />
    </PressButton>
  );
}

/** A small link to another page of this project, e.g. "Blueprint ↗". */
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
