import { LoaderCircle } from "lucide-react";
import type { ReactNode } from "react";

/** Layout pieces shared by the Verification sheet's sections. */

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5">
      <h3 className="text-[13.5px] font-semibold">{title}</h3>
      {children}
    </section>
  );
}

export function Subheading({ children }: { children: ReactNode }) {
  return <h4 className="text-[12px] font-medium text-muted-foreground">{children}</h4>;
}

export function Loading({ label }: { label: string }) {
  return (
    <div className="flex justify-center py-4">
      <LoaderCircle aria-label={label} className="size-4 animate-spin text-muted-foreground" />
    </div>
  );
}

export function Muted({ children }: { children: ReactNode }) {
  return <p className="text-[12.5px] leading-snug text-muted-foreground">{children}</p>;
}
