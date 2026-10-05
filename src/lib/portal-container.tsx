import { createContext, useContext } from "react"

/**
 * Where modal surfaces (Dialog, Sheet) portal to: the window card's modal layer, set by
 * `WindowFrame`. null (outside a frame) falls back to `<body>`.
 *
 * The transparent, undecorated window draws its own card inside a margin. Portaling to
 * `<body>` would put the overlay over that margin too; the modal layer is the containing
 * block for `fixed` children and is clipped by the card's rounded corners.
 */
export const PortalContainerContext = createContext<HTMLElement | null>(null)

export function usePortalContainer(): HTMLElement | undefined {
  return useContext(PortalContainerContext) ?? undefined
}
