import { currentMonitor, cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { useEffect, useRef, useState, type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";
import { Apptitlebar } from "./AppTitlebar";

type Dir =
  | "North" | "South" | "East" | "West"
  | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";

const MIN_W = 640;
const MIN_H = 400;
const MARGIN = 24;      // your p-6
const SLACK = 8;        // resize handles stick out ~8px past the card corners
const INSET = MARGIN - SLACK;


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


function useFlat() {
  const [flat, setFlat] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let timer: ReturnType<typeof setTimeout>;

    const update = async () => {
      if ((await win.isMaximized()) || (await win.isFullscreen())) {
        setFlat(true);
        return;
      }
      const monitor = await currentMonitor();
      if (!monitor) return setFlat(false);

      const size = await win.outerSize();
      const { width, height } = monitor.workArea.size;
      const tol = 16; // physical px

      // >= (not abs) so a window that overshoots the work area still counts
      const fullW = size.width >= width - tol;
      const fullH = size.height >= height - tol;
      console.log("flat check", { size, width, height, fullW, fullH });
      setFlat(fullW || fullH);
    };

    // snaps can resize in more than one step, so re-check once things settle
    const schedule = () => {
      update();
      clearTimeout(timer);
      timer = setTimeout(update, 120);
    };

    schedule();
    const a = win.onResized(schedule);
    const b = win.onMoved(schedule);
    return () => {
      clearTimeout(timer);
      a.then((f) => f());
      b.then((f) => f());
    };
  }, []);

  return flat;
}


function useClickThroughMargin(flat: boolean) {
  const flatRef = useRef(flat);
  flatRef.current = flat;

  useEffect(() => {
    const win = getCurrentWindow();
    let ignoring = false;
    let pressed = false;
    let pos = { x: 0, y: 0 };
    let size = { width: 0, height: 0 };

    const setIgnore = (v: boolean) => {
      if (v === ignoring) return;
      ignoring = v;
      win.setIgnoreCursorEvents(v);
    };

    const refresh = async () => {
      pos = await win.outerPosition();
      size = await win.outerSize();
    };
    refresh();
    const unMove = win.onMoved(refresh);
    const unResize = win.onResized(refresh);

    const down = () => (pressed = true);
    const up = () => (pressed = false);
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("pointerup", up, true);
    window.addEventListener("pointercancel", up, true);

    const tick = async () => {
      // never flip to click-through mid-drag, or drags/resizes can break
      if (pressed) return;
      if (flatRef.current) return setIgnore(false);

      const c = await cursorPosition();
      const dpr = window.devicePixelRatio;
      const x = (c.x - pos.x) / dpr;
      const y = (c.y - pos.y) / dpr;
      const w = size.width / dpr;
      const h = size.height / dpr;

      const inside =
        x >= INSET && x <= w - INSET && y >= INSET && y <= h - INSET;
      setIgnore(!inside);
    };

    const id = setInterval(tick, 30);

    return () => {
      clearInterval(id);
      unMove.then((f) => f());
      unResize.then((f) => f());
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("pointerup", up, true);
      window.removeEventListener("pointercancel", up, true);
      win.setIgnoreCursorEvents(false);
    };
  }, []);
}


export const WindowFrame: React.FC<HTMLAttributes<HTMLDivElement>> = ({
  children,
  className,
  ...props
}) => {
  const flat = useFlat();
  useClickThroughMargin(flat);

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
