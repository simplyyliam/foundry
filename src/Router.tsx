import { createBrowserRouter, Navigate, type RouteObject } from "react-router-dom";
import type { ComponentType } from "react";
import { AppLayout } from "./pages/app";
import { StandaloneLayout } from "./pages/standalone";

const DEFAULT_ROUTE = "/home";

type PageModule = { default: ComponentType };

// Lazy glob: each page becomes its own chunk, loaded on first navigation.
const pages = import.meta.glob<PageModule>("./pages/**/page.tsx");

/**
 * ./pages/app/users/[id]/page.tsx        -> /users/:id
 * ./pages/standalone/docs/[...rest]/page.tsx -> /docs/*
 */
function toRoutePath(file: string) {
  const path = file
    .replace(/^\.\/pages\/(app|standalone)/, "")
    .replace(/\/page\.tsx$/, "")
    .replace(/\[\.\.\.[^\]]+\]/g, "*")
    .replace(/\[([^\]]+)\]/g, ":$1");

  return path || "/";
}

const hasAppIndex = Object.keys(pages).some(f => f === "./pages/app/page.tsx");
const appRoutes: RouteObject[] = hasAppIndex
  ? []
  : [{ index: true, element: <Navigate to={DEFAULT_ROUTE} replace /> }];

const standaloneRoutes: RouteObject[] = [];

for (const [file, load] of Object.entries(pages)) {
  const route: RouteObject = {
    path: toRoutePath(file),
    lazy: async () => ({ Component: (await load()).default }),
  };

  (file.startsWith("./pages/standalone/") ? standaloneRoutes : appRoutes).push(route);
}

export const RouterBrowser = createBrowserRouter([
  { path: "/", element: <AppLayout />, children: appRoutes },
  { element: <StandaloneLayout />, children: standaloneRoutes },
  { path: "*", element: <Navigate to={DEFAULT_ROUTE} replace /> },
]);