import { useId, type ReactNode } from "react";
import { cn } from "@/lib/utils";

interface PromptCardProps {
  title: string;
  description: ReactNode;
  /** Dims the card; its controls are disabled by the caller. */
  disabled?: boolean;
  children: ReactNode;
}

/** One of the two generate cards: a title, one explaining line, then its controls. */
export function PromptCard({ title, description, disabled, children }: PromptCardProps) {
  const id = useId();
  return (
    <section
      aria-labelledby={id}
      aria-disabled={disabled || undefined}
      className="flex flex-col gap-4 rounded-xl bg-card p-5 ring-1 ring-foreground/8"
    >
      <header className="flex flex-col gap-1">
        <h2 id={id} className={cn("text-[14px] font-semibold", disabled && "text-muted-foreground")}>
          {title}
        </h2>
        <p className="text-[13px] leading-snug text-pretty text-muted-foreground">{description}</p>
      </header>
      {children}
    </section>
  );
}
