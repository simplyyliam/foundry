import { matchPath, useLocation } from "react-router-dom";
import { PROJECT_VIEW_IDS, useProjectView, type ProjectViewId } from "./projectView";
import { PANEL_VIEWS, ROUTE_VIEWS } from "./views";
import { ViewSwitcher } from "./ViewSwitcher";

const PROJECT_VIEW_TITLES = Object.fromEntries(
  PROJECT_VIEW_IDS.map((id) => [id, PANEL_VIEWS[id].title]),
) as Record<ProjectViewId, string>;

const isProjectView = (id: string): id is ProjectViewId => (PROJECT_VIEW_IDS as readonly string[]).includes(id);

/**
 * The right-hand contextual panel: shows the view mapped to the current route (see
 * `ROUTE_VIEWS`), or nothing. On project routes the title is a dropdown between the project
 * views (Roadmap, References), and the session's choice replaces the route's default. Other
 * routes get no switcher. Each view widget draws its own panel chrome and header; the frame
 * only composes. A project view's project comes from the route, so it always matches the page.
 */
export function ContextualPanelFrame() {
  const { pathname } = useLocation();
  const chosen = useProjectView();

  for (const route of ROUTE_VIEWS) {
    const match = matchPath(route.path, pathname);
    if (!match) continue;
    const projectId = match.params.projectId ?? null;
    const switchable = isProjectView(route.view) && projectId !== null;
    const view = PANEL_VIEWS[switchable ? chosen : route.view];
    if (!view.component || (view.needsProject && !projectId)) return null;
    const View = view.component;
    const title = switchable ? <ViewSwitcher current={chosen} titles={PROJECT_VIEW_TITLES} /> : undefined;
    // Remount per view and project, so view-local state (e.g. an open pop-out) never leaks.
    return <View key={`${view.id}:${projectId ?? ""}`} projectId={projectId} title={title} />;
  }
  return null;
}
