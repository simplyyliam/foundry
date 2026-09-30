import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import type { LucideIcon } from "lucide-react";

/** A Hugeicons icon or a lucide icon component. */
export type IconType = IconSvgElement | LucideIcon;

export interface IconProps {
  icon: IconType;
  /** Size in px. Omit to size it with CSS (e.g. a parent's `[&_svg]:size-4`). */
  size?: number;
  className?: string;
}

function isHugeicon(icon: IconType): icon is IconSvgElement {
  return Array.isArray(icon);
}

/** Renders either icon library through one API. Decorative by default. */
export function Icon({ icon, size, className }: IconProps) {
  if (isHugeicon(icon)) {
    return <HugeiconsIcon icon={icon} size={size} className={className} aria-hidden />;
  }
  const LucideComponent = icon;
  return <LucideComponent size={size} className={className} aria-hidden />;
}
