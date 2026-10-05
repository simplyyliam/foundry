import type { ComponentProps } from "react";
import { Button } from "@/components/ui/button";
import { usePressScale } from "../hook/usePressScale";
import { mergeRefs } from "../utils/merge-refs";

export interface PressButtonProps extends ComponentProps<typeof Button> {
  /** Scale while pressed (see `usePressScale`). */
  pressScale?: number;
}

/**
 * The shadcn `Button` with the press-scale feedback built in. Handy where a button mounts
 * later than its parent (inside a dialog, or only in some states), since the feedback then
 * attaches when the button itself mounts.
 */
export function PressButton({ pressScale, ref, ...props }: PressButtonProps) {
  const pressRef = usePressScale<HTMLButtonElement>({ scale: pressScale });
  return <Button ref={mergeRefs(pressRef, ref)} {...props} />;
}
