import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";
import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn } from "@/lib/utils";

const HIDE_DELAY = 150;

export const Apptitlebar = () => {
  const [visible, setVisible] = useState(false);
  const hideTimeout = useRef<ReturnType<typeof setTimeout>>(undefined);

  const show = () => {
    clearTimeout(hideTimeout.current);
    setVisible(true);
  };

  const hide = () => {
    clearTimeout(hideTimeout.current);
    hideTimeout.current = setTimeout(() => setVisible(false), HIDE_DELAY);
  };

  useEffect(() => () => clearTimeout(hideTimeout.current), []);

  const btn =
    "flex cursor-pointer items-center justify-center px-4 py-2 transition-colors ease-linear hover:bg-neutral-300";

  return (
    <div
      className={cn(
        "relative -mx-1.5 -mt-1.5 shrink-0 pb-1.5",
        visible ? "pb-0" : "pb-1.5"
      )}
      onMouseEnter={show}
      onMouseLeave={hide}
    >
      {/* hover zone: starts at the card's top edge */}
      <div
        className={cn(
          "absolute inset-x-0 top-0 z-40",
          visible ? "h-0" : "h-2.5"
        )}
      />

      <motion.div
        initial={false}
        animate={{ height: visible ? "auto" : 0 }}
        transition={{ duration: 0.28, ease: [0.32, 0.72, 0, 1] }}
        className="overflow-hidden"
      >
        <div
          data-tauri-drag-region
          className="flex w-full select-none items-center justify-between"
        >
          <span />
          <div className="flex items-center">
            <button onClick={() => getCurrentWindow().minimize()} className={btn}>
              <Minus size={16} />
            </button>
            <button onClick={() => getCurrentWindow().toggleMaximize()} className={btn}>
              <Square size={16} />
            </button>
            <button onClick={() => getCurrentWindow().close()} className={btn}>
              <X size={16} />
            </button>
          </div>
        </div>
      </motion.div>
    </div>
  );
};