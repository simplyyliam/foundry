import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useEffectEvent,
  useMemo,
  useState,
  type ComponentProps,
  type CSSProperties,
} from "react";
import { mergeProps } from "@base-ui/react/merge-props";
import { useRender } from "@base-ui/react/use-render";
import { cva, type VariantProps } from "class-variance-authority";
import { PanelLeftIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/**
 * In-flow sidebar with the same API as shadcn/ui's sidebar.
 *
 * Unlike shadcn's, nothing here is `position: fixed` or sized to the viewport:
 * the sidebar takes space in its parent's flex row, so it stays inside
 * whatever container it's rendered in (e.g. the WindowFrame card).
 */

const SIDEBAR_WIDTH = "16rem";
const SIDEBAR_WIDTH_ICON = "3rem";
/** Space between a floating/inset sidebar and the content. Matches WindowFrame's p-1.5. */
const SIDEBAR_GAP = "0.375rem";
const SIDEBAR_KEYBOARD_SHORTCUT = "mod+b";

const TRANSITION = "duration-200 ease-[cubic-bezier(0.32,0.72,0,1)] motion-reduce:transition-none";

type SidebarState = "expanded" | "collapsed";

type SidebarContextProps = {
  state: SidebarState;
  open: boolean;
  setOpen: (open: boolean | ((open: boolean) => boolean)) => void;
  toggleSidebar: () => void;
};

const SidebarContext = createContext<SidebarContextProps | null>(null);

function useSidebar() {
  const context = useContext(SidebarContext);
  if (!context) {
    throw new Error("useSidebar must be used within a SidebarProvider.");
  }
  return context;
}

function readStoredOpen(key: string | undefined): boolean | undefined {
  if (!key) return undefined;
  try {
    const value = localStorage.getItem(key);
    return value === null ? undefined : value === "true";
  } catch {
    return undefined;
  }
}

type Shortcut = {
  key: string;
  mod: boolean;
  ctrl: boolean;
  meta: boolean;
  alt: boolean;
  shift: boolean;
};

/** Parses e.g. "mod+b", "ctrl+alt+b". `mod` is Ctrl on Windows/Linux, Cmd on macOS. */
function parseShortcut(shortcut: string): Shortcut {
  const parts = shortcut.toLowerCase().split("+").map((part) => part.trim());
  const key = parts.pop() ?? "";
  const has = (...names: string[]) => names.some((name) => parts.includes(name));
  return {
    key,
    mod: has("mod"),
    ctrl: has("ctrl", "control"),
    meta: has("meta", "cmd"),
    alt: has("alt", "option"),
    shift: has("shift"),
  };
}

/** Modifiers must match exactly, so "mod+b" doesn't also fire on "mod+alt+b". */
function matchesShortcut(event: KeyboardEvent, shortcut: Shortcut): boolean {
  const modifiersMatch = shortcut.mod
    ? event.ctrlKey !== event.metaKey // exactly one of Ctrl/Cmd
    : event.ctrlKey === shortcut.ctrl && event.metaKey === shortcut.meta;
  if (!modifiersMatch || event.altKey !== shortcut.alt || event.shiftKey !== shortcut.shift) {
    return false;
  }
  // Letters and digits match on the physical key, since Alt/Option changes
  // `event.key` (e.g. Option+B gives "∫" on macOS).
  if (/^[a-z]$/.test(shortcut.key)) return event.code === `Key${shortcut.key.toUpperCase()}`;
  if (/^[0-9]$/.test(shortcut.key)) return event.code === `Digit${shortcut.key}`;
  return event.key.toLowerCase() === shortcut.key;
}

function SidebarProvider({
  defaultOpen = true,
  open: openProp,
  onOpenChange,
  keyboardShortcut = SIDEBAR_KEYBOARD_SHORTCUT,
  storageKey,
  className,
  style,
  children,
  ...props
}: ComponentProps<"div"> & {
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /**
   * Toggle shortcut, e.g. "mod+b" (default) or "mod+alt+b". `mod` is Ctrl on
   * Windows/Linux and Cmd on macOS. Pass `null` to disable.
   */
  keyboardShortcut?: string | null;
  /** Persist the open state to localStorage under this key. */
  storageKey?: string;
}) {
  const [internalOpen, setInternalOpen] = useState(
    () => readStoredOpen(storageKey) ?? defaultOpen,
  );
  const isControlled = openProp !== undefined;
  const open = isControlled ? openProp : internalOpen;

  const setOpen = useCallback(
    (value: boolean | ((open: boolean) => boolean)) => {
      if (isControlled) {
        onOpenChange?.(typeof value === "function" ? value(openProp) : value);
        return;
      }
      setInternalOpen((prev) => {
        const next = typeof value === "function" ? value(prev) : value;
        if (next !== prev) onOpenChange?.(next);
        return next;
      });
    },
    [isControlled, openProp, onOpenChange],
  );

  const toggleSidebar = useCallback(() => setOpen((prev) => !prev), [setOpen]);

  useEffect(() => {
    if (!storageKey) return;
    try {
      localStorage.setItem(storageKey, String(open));
    } catch {
      // Storage unavailable; state just won't persist.
    }
  }, [storageKey, open]);

  const shortcut = useMemo(
    () => (keyboardShortcut ? parseShortcut(keyboardShortcut) : null),
    [keyboardShortcut],
  );

  const onShortcut = useEffectEvent((event: KeyboardEvent) => {
    if (shortcut && !event.repeat && matchesShortcut(event, shortcut)) {
      event.preventDefault();
      toggleSidebar();
    }
  });

  useEffect(() => {
    if (!shortcut) return;
    window.addEventListener("keydown", onShortcut);
    return () => window.removeEventListener("keydown", onShortcut);
  }, [shortcut]);

  const state: SidebarState = open ? "expanded" : "collapsed";

  const contextValue = useMemo<SidebarContextProps>(
    () => ({ state, open, setOpen, toggleSidebar }),
    [state, open, setOpen, toggleSidebar],
  );

  return (
    <SidebarContext.Provider value={contextValue}>
      <div
        data-slot="sidebar-wrapper"
        style={
          {
            "--sidebar-width": SIDEBAR_WIDTH,
            "--sidebar-width-icon": SIDEBAR_WIDTH_ICON,
            "--sidebar-gap": SIDEBAR_GAP,
            ...style,
          } as CSSProperties
        }
        // flex-1 lets a provider nest inside another's row (e.g. a right-hand panel).
        className={cn("group/sidebar-wrapper flex size-full min-h-0 min-w-0 flex-1", className)}
        {...props}
      >
        {children}
      </div>
    </SidebarContext.Provider>
  );
}

function Sidebar({
  side = "left",
  variant = "sidebar",
  collapsible = "offcanvas",
  className,
  children,
  ...props
}: ComponentProps<"div"> & {
  side?: "left" | "right";
  variant?: "sidebar" | "floating" | "inset";
  collapsible?: "offcanvas" | "icon" | "none";
}) {
  const { state } = useSidebar();

  if (collapsible === "none") {
    return (
      <div
        data-slot="sidebar"
        data-variant={variant}
        data-side={side}
        className={cn(
          "flex h-full w-(--sidebar-width) shrink-0 flex-col bg-sidebar text-sidebar-foreground",
          className,
        )}
        {...props}
      >
        {children}
      </div>
    );
  }

  const gapped = variant === "floating" || variant === "inset";

  // Expanded and icon widths. Floating/inset include the gap to the content,
  // so it collapses together with the sidebar.
  const width = gapped
    ? "w-[calc(var(--sidebar-width)+var(--sidebar-gap))]"
    : "w-(--sidebar-width)";
  const iconWidth = gapped
    ? "w-[calc(var(--sidebar-width-icon)+var(--sidebar-gap))]"
    : "w-(--sidebar-width-icon)";

  return (
    <div
      data-slot="sidebar"
      data-state={state}
      data-collapsible={state === "collapsed" ? collapsible : ""}
      data-variant={variant}
      data-side={side}
      className={cn(
        // Reserves the sidebar's space in the flex row; the width animation is
        // what makes the content (SidebarInset) grow and shrink alongside it.
        "group peer relative h-full shrink-0 overflow-hidden text-sidebar-foreground contain-paint",
        "transition-[width]",
        TRANSITION,
        // This element is the `group`, so it matches its own data attributes.
        width,
        state === "collapsed" && collapsible === "icon" && iconWidth,
        state === "collapsed" && collapsible === "offcanvas" && "w-0",
      )}
    >
      <div
        data-slot="sidebar-container"
        data-side={side}
        className={cn(
          // Keeps its full width while offcanvas-collapsing and slides out with
          // a transform, so its contents never reflow mid-animation.
          "absolute inset-y-0 flex transition-[translate,width]",
          TRANSITION,
          width,
          state === "collapsed" && collapsible === "icon" && iconWidth,
          side === "left"
            ? "left-0 group-data-[collapsible=offcanvas]:-translate-x-full"
            : "right-0 group-data-[collapsible=offcanvas]:translate-x-full",
          gapped && (side === "left" ? "pr-(--sidebar-gap)" : "pl-(--sidebar-gap)"),
          className,
        )}
        {...props}
      >
        <div
          data-sidebar="sidebar"
          data-slot="sidebar-inner"
          className={cn(
            "flex size-full min-w-0 flex-col",
            variant === "sidebar" &&
              cn("bg-sidebar border-sidebar-border", side === "left" ? "border-r" : "border-l"),
            variant === "floating" &&
              "rounded-[9px] bg-sidebar shadow-sm ring-1 ring-sidebar-border",
            // Inset: a card matching SidebarInset, separated from it by the gap.
            variant === "inset" && "rounded-[9px] bg-sidebar",
          )}
        >
          {children}
        </div>
      </div>
    </div>
  );
}

function SidebarTrigger({ className, onClick, ...props }: ComponentProps<typeof Button>) {
  const { toggleSidebar, open } = useSidebar();

  return (
    <Button
      data-sidebar="trigger"
      data-slot="sidebar-trigger"
      variant="ghost"
      size="icon-sm"
      aria-expanded={open}
      className={className}
      onClick={(event) => {
        onClick?.(event);
        toggleSidebar();
      }}
      {...props}
    >
      <PanelLeftIcon />
      <span className="sr-only">Toggle Sidebar</span>
    </Button>
  );
}

/** Clickable strip on the sidebar's inner edge that toggles it. */
function SidebarRail({ className, ...props }: ComponentProps<"button">) {
  const { toggleSidebar } = useSidebar();

  return (
    <button
      data-sidebar="rail"
      data-slot="sidebar-rail"
      aria-label="Toggle Sidebar"
      tabIndex={-1}
      onClick={toggleSidebar}
      title="Toggle Sidebar"
      className={cn(
        "absolute inset-y-0 z-20 w-3",
        "group-data-[side=left]:right-0 group-data-[side=left]:cursor-w-resize",
        "group-data-[side=right]:left-0 group-data-[side=right]:cursor-e-resize",
        // Hover line centred in the gap; the plain variant already has a border.
        "after:absolute after:inset-y-0 after:w-0.5 after:transition-colors hover:after:bg-sidebar-border group-data-[variant=sidebar]:after:hidden",
        "group-data-[side=left]:after:right-[calc(var(--sidebar-gap)/2-1px)] group-data-[side=right]:after:left-[calc(var(--sidebar-gap)/2-1px)]",
        "group-data-[collapsible=icon]:group-data-[side=left]:cursor-e-resize group-data-[collapsible=icon]:group-data-[side=right]:cursor-w-resize",
        className,
      )}
      {...props}
    />
  );
}

function SidebarInset({ className, ...props }: ComponentProps<"main">) {
  return (
    <main
      data-slot="sidebar-inset"
      className={cn(
        "relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background",
        // Card look when any enclosing provider has an inset sidebar (nested
        // providers put the inset one level deeper than the left sidebar).
        "[[data-slot=sidebar-wrapper]:has(>[data-variant=inset])_&]:rounded-[9px]",
        className,
      )}
      {...props}
    />
  );
}

function SidebarInput({ className, ...props }: ComponentProps<typeof Input>) {
  return (
    <Input
      data-slot="sidebar-input"
      data-sidebar="input"
      className={cn("h-8 w-full bg-input/50 shadow-none", className)}
      {...props}
    />
  );
}

function SidebarHeader({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-header"
      data-sidebar="header"
      className={cn("flex flex-col gap-2 p-2 [--radius:var(--radius-xl)]", className)}
      {...props}
    />
  );
}

function SidebarFooter({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-footer"
      data-sidebar="footer"
      className={cn("flex flex-col gap-2 p-2", className)}
      {...props}
    />
  );
}

function SidebarSeparator({ className, ...props }: ComponentProps<typeof Separator>) {
  return (
    <Separator
      data-slot="sidebar-separator"
      data-sidebar="separator"
      className={cn("mx-2 w-auto bg-sidebar-border", className)}
      {...props}
    />
  );
}

function SidebarContent({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-content"
      data-sidebar="content"
      className={cn(
        "flex min-h-0 flex-1 flex-col gap-2 overflow-auto overscroll-contain [scrollbar-width:none] [--radius:var(--radius-xl)] group-data-[collapsible=icon]:overflow-hidden",
        className,
      )}
      {...props}
    />
  );
}

function SidebarGroup({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group"
      data-sidebar="group"
      className={cn("relative flex w-full min-w-0 flex-col p-2", className)}
      {...props}
    />
  );
}

function SidebarGroupLabel({
  className,
  render,
  ...props
}: useRender.ComponentProps<"div"> & ComponentProps<"div">) {
  return useRender({
    defaultTagName: "div",
    props: mergeProps<"div">(
      {
        className: cn(
          "flex h-8 shrink-0 items-center rounded-xl px-3 text-xs font-medium text-sidebar-foreground/70 ring-sidebar-ring outline-hidden transition-[margin,opacity] duration-200 ease-linear group-data-[collapsible=icon]:-mt-8 group-data-[collapsible=icon]:opacity-0 focus-visible:ring-2 [&>svg]:size-4 [&>svg]:shrink-0",
          className,
        ),
      },
      props,
    ),
    render,
    state: { slot: "sidebar-group-label", sidebar: "group-label" },
  });
}

function SidebarGroupAction({
  className,
  render,
  ...props
}: useRender.ComponentProps<"button"> & ComponentProps<"button">) {
  return useRender({
    defaultTagName: "button",
    props: mergeProps<"button">(
      {
        className: cn(
          "absolute top-3.5 right-3 flex aspect-square w-5 items-center justify-center rounded-xl p-0 text-sidebar-foreground ring-sidebar-ring outline-hidden transition-transform group-data-[collapsible=icon]:hidden after:absolute after:-inset-2 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 md:after:hidden [&>svg]:size-4 [&>svg]:shrink-0",
          className,
        ),
      },
      props,
    ),
    render,
    state: { slot: "sidebar-group-action", sidebar: "group-action" },
  });
}

function SidebarGroupContent({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group-content"
      data-sidebar="group-content"
      className={cn("w-full text-sm", className)}
      {...props}
    />
  );
}

function SidebarMenu({ className, ...props }: ComponentProps<"ul">) {
  return (
    <ul
      data-slot="sidebar-menu"
      data-sidebar="menu"
      className={cn("flex w-full min-w-0 flex-col gap-0.5", className)}
      {...props}
    />
  );
}

function SidebarMenuItem({ className, ...props }: ComponentProps<"li">) {
  return (
    <li
      data-slot="sidebar-menu-item"
      data-sidebar="menu-item"
      className={cn("group/menu-item relative", className)}
      {...props}
    />
  );
}

const sidebarMenuButtonVariants = cva(
  "peer/menu-button group/menu-button flex w-full items-center gap-2 overflow-hidden rounded-xl px-3 py-2 text-left text-sm ring-sidebar-ring outline-hidden transition-[width,height,padding] group-has-data-[sidebar=menu-action]/menu-item:pr-8 group-data-[collapsible=icon]:size-8! group-data-[collapsible=icon]:p-2! hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 active:bg-sidebar-accent active:text-sidebar-accent-foreground disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 data-open:hover:bg-sidebar-accent data-open:hover:text-sidebar-accent-foreground data-active:bg-sidebar-accent data-active:font-medium data-active:text-sidebar-accent-foreground [&_svg]:size-4 [&_svg]:shrink-0 [&>span:last-child]:truncate",
  {
    variants: {
      variant: {
        default: "hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
        outline:
          "bg-background shadow-[0_0_0_1px_var(--sidebar-border)] hover:bg-sidebar-accent hover:text-sidebar-accent-foreground hover:shadow-[0_0_0_1px_var(--sidebar-accent)]",
      },
      size: {
        default: "h-9 text-sm",
        sm: "h-8 text-xs",
        lg: "h-14 px-3 text-sm group-data-[collapsible=icon]:p-0!",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

function SidebarMenuButton({
  render,
  isActive = false,
  variant = "default",
  size = "default",
  tooltip,
  className,
  ...props
}: useRender.ComponentProps<"button"> &
  ComponentProps<"button"> & {
    isActive?: boolean;
    /** Shown only while the sidebar is collapsed to icons. */
    tooltip?: string | ComponentProps<typeof TooltipContent>;
  } & VariantProps<typeof sidebarMenuButtonVariants>) {
  const { state } = useSidebar();
  const button = useRender({
    defaultTagName: "button",
    props: mergeProps<"button">(
      { className: cn(sidebarMenuButtonVariants({ variant, size }), className) },
      props,
    ),
    render: tooltip ? <TooltipTrigger render={render} /> : render,
    state: {
      slot: "sidebar-menu-button",
      sidebar: "menu-button",
      size,
      active: isActive,
    },
  });

  if (!tooltip) return button;

  const tooltipProps = typeof tooltip === "string" ? { children: tooltip } : tooltip;

  return (
    <Tooltip>
      {button}
      <TooltipContent
        side="right"
        align="center"
        hidden={state !== "collapsed"}
        {...tooltipProps}
      />
    </Tooltip>
  );
}

function SidebarMenuAction({
  className,
  render,
  showOnHover = false,
  ...props
}: useRender.ComponentProps<"button"> &
  ComponentProps<"button"> & {
    showOnHover?: boolean;
  }) {
  return useRender({
    defaultTagName: "button",
    props: mergeProps<"button">(
      {
        className: cn(
          "absolute top-1.5 right-1 flex aspect-square w-5 items-center justify-center rounded-xl p-0 text-sidebar-foreground ring-sidebar-ring outline-hidden transition-transform group-data-[collapsible=icon]:hidden peer-hover/menu-button:text-sidebar-accent-foreground peer-data-[size=default]/menu-button:top-2 peer-data-[size=lg]/menu-button:top-2.5 peer-data-[size=sm]/menu-button:top-1 after:absolute after:-inset-2 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 md:after:hidden [&>svg]:size-4 [&>svg]:shrink-0",
          showOnHover &&
            "opacity-0 group-focus-within/menu-item:opacity-100 group-hover/menu-item:opacity-100 peer-data-active/menu-button:text-sidebar-accent-foreground aria-expanded:opacity-100",
          className,
        ),
      },
      props,
    ),
    render,
    state: { slot: "sidebar-menu-action", sidebar: "menu-action" },
  });
}

function SidebarMenuBadge({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-menu-badge"
      data-sidebar="menu-badge"
      className={cn(
        "pointer-events-none absolute right-1 flex h-5 min-w-5 items-center justify-center rounded-xl px-1 text-xs font-medium text-sidebar-foreground tabular-nums select-none group-data-[collapsible=icon]:hidden peer-hover/menu-button:text-sidebar-accent-foreground peer-data-[size=default]/menu-button:top-1.5 peer-data-[size=lg]/menu-button:top-2.5 peer-data-[size=sm]/menu-button:top-1 peer-data-active/menu-button:text-sidebar-accent-foreground",
        className,
      )}
      {...props}
    />
  );
}

function SidebarMenuSkeleton({
  className,
  showIcon = false,
  ...props
}: ComponentProps<"div"> & {
  showIcon?: boolean;
}) {
  // Random width between 50 and 90%, fixed for the component's lifetime.
  const [width] = useState(() => `${Math.floor(Math.random() * 40) + 50}%`);

  return (
    <div
      data-slot="sidebar-menu-skeleton"
      data-sidebar="menu-skeleton"
      className={cn("flex h-8 items-center gap-2 rounded-xl px-2", className)}
      {...props}
    >
      {showIcon && <Skeleton className="size-4 rounded-xl" data-sidebar="menu-skeleton-icon" />}
      <Skeleton
        className="h-4 max-w-(--skeleton-width) flex-1"
        data-sidebar="menu-skeleton-text"
        style={{ "--skeleton-width": width } as CSSProperties}
      />
    </div>
  );
}

function SidebarMenuSub({ className, ...props }: ComponentProps<"ul">) {
  return (
    <ul
      data-slot="sidebar-menu-sub"
      data-sidebar="menu-sub"
      className={cn(
        "mx-3.5 flex min-w-0 translate-x-px flex-col gap-1 border-l border-sidebar-border px-2.5 py-0.5 group-data-[collapsible=icon]:hidden",
        className,
      )}
      {...props}
    />
  );
}

function SidebarMenuSubItem({ className, ...props }: ComponentProps<"li">) {
  return (
    <li
      data-slot="sidebar-menu-sub-item"
      data-sidebar="menu-sub-item"
      className={cn("group/menu-sub-item relative", className)}
      {...props}
    />
  );
}

function SidebarMenuSubButton({
  render,
  size = "md",
  isActive = false,
  className,
  ...props
}: useRender.ComponentProps<"a"> &
  ComponentProps<"a"> & {
    size?: "sm" | "md";
    isActive?: boolean;
  }) {
  return useRender({
    defaultTagName: "a",
    props: mergeProps<"a">(
      {
        className: cn(
          "flex h-7 min-w-0 -translate-x-px items-center gap-2 overflow-hidden rounded-xl px-3 text-sidebar-foreground ring-sidebar-ring outline-hidden group-data-[collapsible=icon]:hidden hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 active:bg-sidebar-accent active:text-sidebar-accent-foreground disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 data-[size=md]:text-sm data-[size=sm]:text-xs data-active:bg-sidebar-accent data-active:text-sidebar-accent-foreground [&>span:last-child]:truncate [&>svg]:size-4 [&>svg]:shrink-0 [&>svg]:text-sidebar-accent-foreground",
          className,
        ),
      },
      props,
    ),
    render,
    state: {
      slot: "sidebar-menu-sub-button",
      sidebar: "menu-sub-button",
      size,
      active: isActive,
    },
  });
}

export {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInput,
  SidebarInset,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarProvider,
  SidebarRail,
  SidebarSeparator,
  SidebarTrigger,
  sidebarMenuButtonVariants,
  useSidebar,
};
