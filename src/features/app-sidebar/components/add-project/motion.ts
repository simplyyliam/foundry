import type { Transition, Variants } from "motion/react";

/** Motion tokens for the add-project dialog. Transform and opacity only. */

const EASE_OUT = [0.32, 0.72, 0, 1] as const;

/** Container height follows its content: critically damped, no bounce. */
export const HEIGHT: Transition = { type: "spring", visualDuration: 0.32, bounce: 0 };

/** Whole-view swap (search ⇄ folder browser): a quick crossfade with a hint of travel. */
export const view: Variants = {
  hidden: (direction: 1 | -1) => ({ opacity: 0, x: 12 * direction, filter: "blur(2px)" }),
  shown: {
    opacity: 1,
    x: 0,
    filter: "blur(0px)",
    transition: { duration: 0.24, ease: EASE_OUT },
    // A lingering blur(0px) keeps a compositing layer and can soften text.
    transitionEnd: { filter: "none" },
  },
  gone: (direction: 1 | -1) => ({
    opacity: 0,
    x: -8 * direction,
    filter: "blur(2px)",
    transition: { duration: 0.12, ease: EASE_OUT },
  }),
};
