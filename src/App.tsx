import { RouterProvider } from "react-router";
import { RouterBrowser } from "./Router";

export default function App() {
  return (
    <RouterProvider router={RouterBrowser}/>
  )
}