import { WindowFrame } from "@/shared";
import { Outlet } from "react-router-dom";

export default function AppLayout() {
  return (
    <WindowFrame>
      <Outlet/>
    </WindowFrame>
  )
}