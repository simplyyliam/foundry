import { useState, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { PortalContainerContext } from "@/lib/portal-container";

export interface WindowFrameProps extends HTMLAttributes<HTMLDivElement> {
  /** Rendered at the top of the card, above the content. */
  titlebar?: ReactNode;
  /** Classes for the card (background, padding). */
  cardClassName?: string;
}

/**
 * The window's content card for an undecorated Tauri window. It fills the window edge to
 * edge: the OS rounds the corners and Tauri handles resizing at the window border.
 *
 * `className` and other div props apply to the content area below the titlebar.
 */
export function WindowFrame({
  titlebar,
  cardClassName = "bg-accent p-(--shell-padding)",
  className,
  children,
  ...props
}: WindowFrameProps) {
  // Dialogs and sheets portal here (see @/lib/portal-container); state so they re-render once it exists.
  const [modalLayer, setModalLayer] = useState<HTMLDivElement | null>(null);

  return (
    <div className={cn("relative flex h-screen flex-col overflow-hidden", cardClassName)}>
      <PortalContainerContext.Provider value={modalLayer}>
        {titlebar}
        <div className={cn("min-h-0 flex-1", className)} {...props}>
          {children}
        </div>
      </PortalContainerContext.Provider>

      {/*
        Modal layer: covers the card exactly. `contain: layout` makes it the containing block
        for `fixed` children, so an overlay's `fixed inset-0` fills the card. It ignores the
        pointer itself; whatever is portaled into it takes clicks again.
      */}
      <div
        ref={setModalLayer}
        data-slot="modal-layer"
        className="pointer-events-none absolute inset-0 z-50 overflow-hidden [contain:layout] *:pointer-events-auto"
      />
    </div>
  );
}
