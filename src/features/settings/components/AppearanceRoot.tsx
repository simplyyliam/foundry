import { MotionConfig } from "motion/react";
import type { ReactNode } from "react";
import { useAppearance } from "../hooks/useAppearance";

/**
 * Applies the saved theme and reduced-motion override app-wide from startup. Wraps the whole
 * app so every motion component and `useReducedMotionConfig()` sees the override.
 */
export function AppearanceRoot({ children }: { children: ReactNode }) {
  const { reduceMotion } = useAppearance();
  return <MotionConfig reducedMotion={reduceMotion ? "always" : "user"}>{children}</MotionConfig>;
}
