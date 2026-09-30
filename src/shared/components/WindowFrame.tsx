import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState, type HTMLAttributes } from "react";

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
    return () => { unlisten.then((f) => f()); };
  }, []);


  return (
    <div className={flat ? "h-screen" : "h-screen p-6"}>
      <div
        className={
          flat
            ? "h-full bg-background overflow-hidden"
            : "h-full bg-background overflow-hidden rounded-[15px] " +
            "shadow-[0_4px_16px_rgba(0,0,0,0.3),0_0_0_1px_rgba(0,0,0,0.15)]"

          }
        {...props}>
        {children}
      </div>
    </div>
  );
}

