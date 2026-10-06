import { useId, type ReactNode } from "react";

/** One section of the Setup page: a heading, a short intro, then its content. */
export function SetupSection({ title, intro, children }: { title: string; intro: ReactNode; children: ReactNode }) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="flex flex-col gap-4">
      <header className="flex flex-col gap-1">
        <h2 id={id} className="text-[15px] font-semibold">
          {title}
        </h2>
        <p className="text-[13px] leading-snug text-pretty text-muted-foreground">{intro}</p>
      </header>
      {children}
    </section>
  );
}
