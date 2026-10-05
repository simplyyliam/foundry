import { useEffect, useRef } from "react";
import { animate, press, type AnimationOptions } from "motion";
import { useReducedMotionConfig } from "motion/react";

/** Quick, damped dip on press. */
const PRESS: AnimationOptions = { type: "spring", visualDuration: 0.12, bounce: 0 };
/** Springs back with a subtle overshoot. */
const RELEASE: AnimationOptions = { type: "spring", visualDuration: 0.35, bounce: 0.45 };

export interface PressScaleOptions {
  /** Scale while pressed. Keep it subtle: 0.95–0.98. */
  scale?: number;
  disabled?: boolean;
}

/**
 * Tactile press feedback: scales the element down while pressed and springs it
 * back with a slight bounce on release. Handles pointer and keyboard (Enter)
 * presses, cancels cleanly when the pointer leaves, and is skipped for users
 * who prefer reduced motion.
 *
 * Animates `transform` only, so it never triggers layout.
 */
export function usePressScale<T extends Element>({
  scale = 0.97,
  disabled = false,
}: PressScaleOptions = {}) {
  const ref = useRef<T>(null);
  const reducedMotion = useReducedMotionConfig();

  useEffect(() => {
    const element = ref.current;
    if (!element || disabled || reducedMotion) return;

    return press(element, (target) => {
      animate(target, { scale }, PRESS);
      return () => {
        animate(target, { scale: 1 }, RELEASE);
      };
    });
  }, [scale, disabled, reducedMotion]);

  return ref;
}
