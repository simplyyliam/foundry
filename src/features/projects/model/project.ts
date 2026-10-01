export interface Project {
  /** URL-safe and unique, e.g. "my-repo" or "my-repo-2". */
  id: string;
  name: string;
  /** Absolute folder path. */
  path: string;
  /** Created as a new repository, or an existing folder that was linked. */
  origin: "created" | "linked";
  /** Epoch ms. */
  createdAt: number;
}

/** A page inside a project, shown as a sidebar sub-item: /projects/:id/<segment>. */
export interface ProjectSection {
  id: string;
  label: string;
  segment: string;
  /** Classes for the item's dot, e.g. its colour. */
  dotClassName: string;
}

/**
 * Sub-items under each project, in order; the first is where a project opens.
 * Add one here plus a page at pages/app/projects/[projectId]/<segment>/page.tsx.
 */
export const PROJECT_SECTIONS: readonly ProjectSection[] = [
  { id: "idea", label: "Idea", segment: "idea", dotClassName: "bg-[#e5b55c]" },
  { id: "blueprint", label: "Blueprint", segment: "blueprints", dotClassName: "bg-[#6b9bd8]" },
];

export const projectPath = (id: string) => `/projects/${encodeURIComponent(id)}`;

export const projectSectionPath = (id: string, section: ProjectSection) =>
  `${projectPath(id)}/${section.segment}`;
