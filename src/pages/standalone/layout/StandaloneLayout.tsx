import { WindowFrame } from "@/shared";
import { Outlet } from "react-router-dom";

export default function StandaloneLayout() {
  return (
    <WindowFrame>
      <Outlet/>
    </WindowFrame>
  )
}