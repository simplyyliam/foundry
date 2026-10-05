import { RouterProvider } from "react-router";
import { RouterBrowser } from "./Router";
import { TooltipProvider } from "./components/ui/tooltip";
import { useEffect } from "react";
import { commands } from "./lib/bindings";
import { AppContextMenu } from "./shared";
import { AppearanceRoot } from "./features/settings";

export default function App() {
  useEffect(() => {
    (async () => {
      try {
        const res = await commands.appInfo()
        if (res.status === "ok") {
          console.log("Backend ready:", res.data.version, res.data.os)
        } else {
          console.log("appInfo returned an error:", res.error.code, res.error.message)
        }
      } catch (err) {
        console.error("IPC call to appInfo failed:", err)
      }
    })()
  }, []);

  return (
    // Theme and reduced motion from settings, applied from startup in every window.
    <AppearanceRoot>
      <TooltipProvider>
        {/* The app-wide right-click menu (replaces the webview's native one). */}
        <AppContextMenu>
          <RouterProvider router={RouterBrowser} />
        </AppContextMenu>
      </TooltipProvider>
    </AppearanceRoot>
  )
}
