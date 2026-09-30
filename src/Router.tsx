import { createBrowserRouter, Navigate, type RouteObject } from "react-router-dom";
import { AppLayout } from "./pages/app";
import { StandaloneLayout } from "./pages/standalone";
import { pages, type PageEntry } from "./routing";

const DEFAULT_ROUTE = "/home";

const toRoute = (page: PageEntry): RouteObject => ({
  path: page.path,
  lazy: async () => ({ Component: (await page.load()).default }),
});

const appPages = pages.filter((page) => page.section === "app");
const standalonePages = pages.filter((page) => page.section === "standalone");

const hasAppIndex = appPages.some((page) => page.path === "/");
const appRoutes: RouteObject[] = [
  ...(hasAppIndex ? [] : [{ index: true, element: <Navigate to={DEFAULT_ROUTE} replace /> }]),
  ...appPages.map(toRoute),
];

export const RouterBrowser = createBrowserRouter([
  { path: "/", element: <AppLayout />, children: appRoutes },
  { element: <StandaloneLayout />, children: standalonePages.map(toRoute) },
  { path: "*", element: <Navigate to={DEFAULT_ROUTE} replace /> },
]);
