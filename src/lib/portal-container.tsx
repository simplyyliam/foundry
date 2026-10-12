import { createContext, useContext } from "react"

/**
 * Where modal surfaces (Dialog, Sheet) portal to: the window card's modal layer, set by
 * `WindowFrame`. null (outside a frame) falls back to `<body>`.
 *
 * The modal layer is the containing block for `fixed` children, so overlays cover the
 * card, which fills the window.
 */
export const PortalContainerContext = createContext<HTMLElement | null>(null)

export function usePortalContainer(): HTMLElement | undefined {
  return useContext(PortalContainerContext) ?? undefined
}
