import { Breadcrumbs } from "@/features/inset-header";
import { usePageHeaderCrumbs, type Crumb } from "@/shared";

export interface InsetHeaderProps {
  /** Shown when the current page hasn't set its own header via `usePageHeader`. */
  fallback: readonly Crumb[];
}

export function InsetHeader({ fallback }: InsetHeaderProps) {
  const crumbs = usePageHeaderCrumbs() ?? fallback;

  return (
    // Fixed height so switching pages never shifts the content below.
    <header data-slot="inset-header" className="flex h-12 shrink-0 items-center px-5">
      <Breadcrumbs crumbs={crumbs} />
    </header>
  );
}
