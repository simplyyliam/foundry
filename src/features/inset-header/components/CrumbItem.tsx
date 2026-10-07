import { ChevronRight } from "lucide-react";
import { motion, useReducedMotionConfig } from "motion/react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { usePressScale, type Crumb } from "@/shared";
import { SubtitleMenu } from "./SubtitleMenu";

const ENTER_EASE = [0.32, 0.72, 0, 1] as const;

function CrumbLink({ to, children }: { to: string; children: ReactNode }) {
  const ref = usePressScale<HTMLAnchorElement>();

  return (
    <Link
      ref={ref}
      to={to}
      className="flex min-w-0 items-baseline gap-2 rounded-md outline-none transition-opacity hover:opacity-65 focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      {children}
    </Link>
  );
}

interface CrumbItemProps {
  crumb: Crumb;
  /** Position in the trail; every crumb after the first gets a separator. */
  index: number;
  /** The current page: never a link. */
  isCurrent: boolean;
}

export function CrumbItem({ crumb, index, isCurrent }: CrumbItemProps) {
  const reducedMotion = useReducedMotionConfig();

  if (crumb.subtitleMenu) {
    // The subtitle is its own control, so the crumb is never a link as well.
    return (
      <motion.li
        initial={reducedMotion ? false : { opacity: 0, x: -4 }}
        animate={{ opacity: 1, x: 0 }}
        transition={{ duration: 0.2, ease: ENTER_EASE }}
        className="flex min-w-0 items-center gap-2"
      >
        {index > 0 && <ChevronRight aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />}
        <span aria-current={isCurrent ? "page" : undefined} className="shrink-0 font-medium text-foreground">
          {crumb.title}
        </span>
        <SubtitleMenu subtitle={crumb.subtitle} menu={crumb.subtitleMenu} />
      </motion.li>
    );
  }

  const content = (
    <>
      <span className="shrink-0 font-medium text-foreground">{crumb.title}</span>
      {crumb.subtitle && <span className="truncate text-muted-foreground">{crumb.subtitle}</span>}
    </>
  );

  return (
    <motion.li
      // Only newly shown crumbs animate; unchanged ones keep their key.
      initial={reducedMotion ? false : { opacity: 0, x: -4 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ duration: 0.2, ease: ENTER_EASE }}
      className="flex min-w-0 items-center gap-2"
    >
      {index > 0 && (
        <ChevronRight aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
      )}
      {crumb.to && !isCurrent ? (
        <CrumbLink to={crumb.to}>{content}</CrumbLink>
      ) : (
        <span aria-current={isCurrent ? "page" : undefined} className="flex min-w-0 items-baseline gap-2">
          {content}
        </span>
      )}
    </motion.li>
  );
}
