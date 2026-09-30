import { WindowFrame } from "@/shared";
import { Outlet } from "react-router-dom";

export default function AppLayout() {
  return (
    <WindowFrame className="flex gap-1.5">
      <div className="flex items-center justify-center h-full w-94 rounded-[9px] bg-background">left panel</div>
      <Outlet/>
      <div className="flex items-center justify-center h-full w-94 rounded-[9px] bg-background">right panel</div>
    </WindowFrame>
  )
}