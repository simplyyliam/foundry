import { RouterProvider } from "react-router";
import { RouterBrowser } from "./Router";
import { TooltipProvider } from "./components/ui/tooltip";

export default function App() {
  return (
    <TooltipProvider>
      <RouterProvider router={RouterBrowser}/>
    </TooltipProvider>
  )
}