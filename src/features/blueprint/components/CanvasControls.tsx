import { Panel, useReactFlow } from "@xyflow/react";
import { Maximize, Minus, Plus, type LucideIcon } from "lucide-react";
import { useReducedMotionConfig } from "motion/react";
import { PressButton } from "@/shared";
import { FIT_VIEW } from "../lib/layout";

/** Zoom out / in / fit, bottom right. Ctrl + scroll and pinch zoom too; scrolling pans. */
export function CanvasControls() {
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const reducedMotion = useReducedMotionConfig();
  const duration = (ms: number) => (reducedMotion ? 0 : ms);
  return (
    <Panel position="bottom-right" className="flex gap-1 rounded-full bg-popover p-1 shadow-md ring-1 ring-foreground/5">
      <ControlButton icon={Minus} label="Zoom out" onClick={() => void zoomOut({ duration: duration(160) })} />
      <ControlButton icon={Plus} label="Zoom in" onClick={() => void zoomIn({ duration: duration(160) })} />
      <ControlButton icon={Maximize} label="Fit to view" onClick={() => void fitView({ ...FIT_VIEW, duration: duration(220) })} />
    </Panel>
  );
}

function ControlButton({ icon: Icon, label, onClick }: { icon: LucideIcon; label: string; onClick: () => void }) {
  return (
    <PressButton pressScale={0.92} variant="ghost" size="icon-sm" aria-label={label} title={label} onClick={onClick}>
      <Icon aria-hidden />
    </PressButton>
  );
}
