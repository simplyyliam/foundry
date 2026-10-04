import { matchPath, useLocation } from "react-router-dom";
import { PANEL_VIEWS, ROUTE_VIEWS } from "./views";

/**
 * The right-hand contextual panel: shows the view mapped to the current route (see
 * `ROUTE_VIEWS`), or nothing. Each view widget draws its own panel chrome and header; the
 * frame only composes. A project view's project comes from the route, so it always matches
 * the page.
 */
export function ContextualPanelFrame() {
  const { pathname } = useLocation();

  for (const route of ROUTE_VIEWS) {
    const match = matchPath(route.path, pathname);
    if (!match) continue;
    const view = PANEL_VIEWS[route.view];
    const projectId = match.params.projectId ?? null;
    if (!view.component || (view.needsProject && !projectId)) return null;
    const View = view.component;
    // Remount per view and project, so view-local state (e.g. an open pop-out) never leaks.
    return <View key={`${view.id}:${projectId ?? ""}`} projectId={projectId} />;
  }
  return null;
}
