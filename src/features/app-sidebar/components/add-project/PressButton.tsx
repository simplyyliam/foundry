import type { ComponentProps } from "react";
import { Button } from "@/components/ui/button";
import { usePressScale } from "@/shared";

/** The shadcn Button with the app's press-scale feedback. */
export function PressButton({ disabled, ...props }: ComponentProps<typeof Button>) {
  const ref = usePressScale<HTMLButtonElement>({ disabled: disabled ?? undefined });
  return <Button ref={ref} disabled={disabled} {...props} />;
}
