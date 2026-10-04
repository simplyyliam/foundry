import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface ArtifactProps {
  title?: ReactNode;
  /** Muted text under the title. */
  description?: ReactNode;
  className?: string;
  children?: ReactNode;
}

/** The gray rounded box that holds a list worth looking at. Callers skip it for empty lists. */
export function Artifact({ title, description, className, children }: ArtifactProps) {
  return (
    <section className={cn("rounded-2xl bg-black/[0.055] px-3.5 py-3 dark:bg-white/[0.06]", className)}>
      {title && <h4 className="text-[13px] leading-snug font-medium text-foreground">{title}</h4>}
      {description && <p className="mt-0.5 text-[12.5px] leading-snug text-muted-foreground">{description}</p>}
      {children && <div className={cn((title || description) && "mt-1.5")}>{children}</div>}
    </section>
  );
}

/** Bulleted list inside an artifact. */
export function Bullets({ items, className }: { items: readonly ReactNode[]; className?: string }) {
  return (
    <ul
      className={cn(
        "flex list-disc flex-col gap-0.5 pl-4.5 text-[13px] leading-snug text-foreground/80 marker:text-foreground/45",
        className,
      )}
    >
      {items.map((item, index) => (
        <li key={index}>{item}</li>
      ))}
    </ul>
  );
}

/** Plain-text section heading (no box). */
export function SectionHeading({ children }: { children: ReactNode }) {
  return <h3 className="text-[13px] leading-snug font-medium text-foreground">{children}</h3>;
}

/** Plain body text (no box). */
export function BodyText({ children, className }: { children: ReactNode; className?: string }) {
  return <p className={cn("text-[13px] leading-[1.45] text-muted-foreground", className)}>{children}</p>;
}

/** Muted secondary line inside an artifact, e.g. "after: …". */
export function MutedLine({ children }: { children: ReactNode }) {
  return <p className="text-[12px] leading-snug text-muted-foreground">{children}</p>;
}
