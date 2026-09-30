import { getCurrentWindow } from "@tauri-apps/api/window";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { useEffect, useState, type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";
import { Apptitlebar } from "./AppTitlebar";

type Dir =
  | "North" | "South" | "East" | "West"
  | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";

const MIN_W = 640;
const MIN_H = 400;

// Card edge sits 24px (p-6) inside the window; each handle straddles that line.
const HANDLES: { dir: Dir; className: string }[] = [
  { dir: "North", className: "top-[20px] left-8 right-8 h-2 cursor-ns-resize" },
  { dir: "South", className: "bottom-[20px] left-8 right-8 h-2 cursor-ns-resize" },
  { dir: "West", className: "left-[20px] top-8 bottom-8 w-2 cursor-ew-resize" },
  { dir: "East", className: "right-[20px] top-8 bottom-8 w-2 cursor-ew-resize" },
  { dir: "NorthWest", className: "top-4 left-4 size-4 cursor-nwse-resize" },
  { dir: "NorthEast", className: "top-4 right-4 size-4 cursor-nesw-resize" },
  { dir: "SouthWest", className: "bottom-4 left-4 size-4 cursor-nesw-resize" },
  { dir: "SouthEast", className: "bottom-4 right-4 size-4 cursor-nwse-resize" },
];

const startResize =
  (dir: Dir) => async (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();

    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId); // must happen before any await

    const win = getCurrentWindow();
    const dpr = window.devicePixelRatio;
    const pos = await win.outerPosition();
    const size = await win.outerSize();
    const sx = e.screenX;
    const sy = e.screenY;
    const minW = MIN_W * dpr;
    const minH = MIN_H * dpr;

    const onMove = (ev: PointerEvent) => {
      const dx = (ev.screenX - sx) * dpr;
      const dy = (ev.screenY - sy) * dpr;
      let { x, y } = pos;
      let w = size.width;
      let h = size.height;

      if (dir.includes("East")) w = Math.max(minW, size.width + dx);
      if (dir.includes("South")) h = Math.max(minH, size.height + dy);
      if (dir.includes("West")) {
        w = Math.max(minW, size.width - dx);
        x = pos.x + size.width - w;
      }
      if (dir.includes("North")) {
        h = Math.max(minH, size.height - dy);
        y = pos.y + size.height - h;
      }

      win.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));
      win.setSize(new PhysicalSize(Math.round(w), Math.round(h)));
    };

    const onUp = () => {
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", onUp);
      el.removeEventListener("pointercancel", onUp);
    };

    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", onUp);
    el.addEventListener("pointercancel", onUp);
  };

export const WindowFrame: React.FC<HTMLAttributes<HTMLDivElement>> = ({
  children,
  className,
  ...props
}) => {
  const [flat, setFlat] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    const update = async () =>
      setFlat((await win.isMaximized()) || (await win.isFullscreen()));
    update();
    const unlisten = win.onResized(update);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className={cn("relative h-screen", !flat && "p-6")}>
      <div
        className={cn(
          "relative flex h-full flex-col overflow-hidden bg-accent p-1.5",
          !flat &&
            "rounded-[15px] shadow-[0_4px_16px_rgba(0,0,0,0.3),0_0_0_1px_rgba(0,0,0,0.15)]"
        )}
        {...props}
      >
        <Apptitlebar />
        <div className={cn("min-h-0 flex-1", className)}>{children}</div>
      </div>
  
      {!flat &&
        HANDLES.map(({ dir, className }) => (
          <div
            key={dir}
            onPointerDown={startResize(dir)}
            className={cn("absolute z-50", className)}
          />
        ))}
    </div>
  );
};