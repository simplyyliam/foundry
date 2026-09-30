import type { Crumb } from "@/shared";
import { CrumbItem } from "./CrumbItem";

/** One crumb renders as a title + subtitle, several as breadcrumbs. */
export function Breadcrumbs({ crumbs }: { crumbs: readonly Crumb[] }) {
  return (
    <nav aria-label="Breadcrumb" className="min-w-0">
      <ol className="flex min-w-0 items-center gap-2 text-[14px] whitespace-nowrap">
        {crumbs.map((crumb, index) => (
          <CrumbItem
            key={`${index}:${crumb.title}`}
            crumb={crumb}
            index={index}
            isCurrent={index === crumbs.length - 1}
          />
        ))}
      </ol>
    </nav>
  );
}
