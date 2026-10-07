import type { ComponentType, ReactNode } from "react";
import { ContextualPanel, ReferencesPanel, RoadmapPanel } from "@/widgets";

export interface PanelViewProps {
  /** The route's `:projectId`, or null. Never null for `needsProject` views. */
  projectId: string | null;
  /** The header title. On project routes it's the switcher between project views. */
  title?: ReactNode;
}

export interface PanelView {
  id: string;
  title: string;
  /** Only shown on routes that carry a `:projectId`. */
  needsProject: boolean;
  /** Null while the view is reserved but not built yet: the route shows no panel. */
  component: ComponentType<PanelViewProps> | null;
}

/** Views the contextual panel can show. A new view is one entry here plus its routes below. */
export const PANEL_VIEWS = {
  roadmap: {
    id: "roadmap",
    title: "Roadmap",
    needsProject: true,
    component: ({ projectId, title }) => (projectId ? <RoadmapPanel projectId={projectId} title={title} /> : null),
  },
  references: {
    id: "references",
    title: "References",
    needsProject: true,
    component: ({ projectId, title }) => (projectId ? <ReferencesPanel projectId={projectId} title={title} /> : null),
  },
  "agent-chats": {
    id: "agent-chats",
    title: "Agent Chats",
    needsProject: false,
    component: () => <ContextualPanel />,
  },
  // Reserved: the ticket system's panel goes here.
  tickets: {
    id: "tickets",
    title: "Tickets",
    needsProject: false,
    component: null,
  },
} satisfies Record<string, PanelView>;

export type PanelViewId = keyof typeof PANEL_VIEWS;

/**
 * Which view each route shows; first match wins. Routes not listed here, like Home, show no
 * panel. On project routes the listed view is only the default: the title dropdown switches
 * between the project views (`PROJECT_VIEW_IDS`), and that choice holds for the session.
 */
export const ROUTE_VIEWS: readonly { path: string; view: PanelViewId }[] = [
  { path: "/projects/:projectId/idea", view: "roadmap" },
  { path: "/projects/:projectId/blueprints", view: "roadmap" },
  { path: "/projects/:projectId/requirements", view: "roadmap" },
  { path: "/projects/:projectId/setup", view: "roadmap" },
  { path: "/projects/:projectId/prompts", view: "roadmap" },
  { path: "/teams/*", view: "agent-chats" },
  { path: "/tickets/*", view: "tickets" },
];
