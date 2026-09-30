// src/shared/components/IconButton.tsx
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import type { LucideIcon } from "lucide-react";
import type { ComponentProps, ReactNode } from "react";

export interface IconButtonProps
  extends Omit<ComponentProps<typeof Button>, "size" | "children"> {
  Label?: string | number;
  children?: ReactNode;
  /** A Hugeicons icon or a lucide icon component. */
  Icon?: IconSvgElement | LucideIcon;
  /** Icon size in px. */
  size?: number;
  stretch?: boolean;
  rounded?: boolean;
  loading?: boolean;
  ariaLabel?: string;
}

function Spinner({ size = 16 }: { size?: number }) {
  return (
    <span
      aria-hidden="true"
      data-icon="inline-start"
      style={{ width: size, height: size }}
      className="inline-block shrink-0 animate-spin rounded-full border-2 border-current border-t-transparent"
    />
  );
}

export function IconButton({
  Label,
  children,
  Icon,
  size,
  stretch = false,
  rounded = false,
  loading = false,
  ariaLabel,
  variant = "secondary",
  type = "button",
  className,
  disabled,
  title,
  ...props
}: IconButtonProps) {
  const content = children ?? Label;
  const hasContent = content != null && content !== false && content !== "";

  const iconProps = {
    size,
    "aria-hidden": true as const,
    "data-icon": hasContent ? ("inline-start" as const) : undefined,
  };

  function isHugeicon(icon: IconSvgElement | LucideIcon): icon is IconSvgElement {
    return Array.isArray(icon);
  }

  let icon: ReactNode = null;
  if (loading) {
    icon = <Spinner size={size} />;
  } else if (Icon) {
    icon = isHugeicon(Icon) ? (
      <HugeiconsIcon icon={Icon} {...iconProps} />
    ) : (
      <Icon {...iconProps} />
    );
  }

  return (
    <Button
      {...props}
      type={type}
      variant={variant}
      size={hasContent ? undefined : "icon"}
      title={title}
      aria-label={ariaLabel ?? props["aria-label"] ?? title}
      aria-busy={loading || undefined}
      disabled={disabled || loading}
      className={cn(
        "flex items-center rounded-xl",
        stretch && "flex-1",
        rounded && "rounded-full",
        className,
      )}
    >
      {icon}
      {hasContent && <span>{content}</span>}
    </Button>
  );
}
