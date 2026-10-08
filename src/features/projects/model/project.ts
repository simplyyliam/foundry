export type { Project } from "@/lib/bindings";

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
  { id: "requirements", label: "Requirements", segment: "requirements", dotClassName: "bg-[#e0895a]" },
  { id: "setup", label: "Setup", segment: "setup", dotClassName: "bg-[#8fb87a]" },
  { id: "prompts", label: "Prompts", segment: "prompts", dotClassName: "bg-[#a98bd4]" },
  { id: "verification", label: "Verification", segment: "verification", dotClassName: "bg-[#5fb3a6]" },
];

export const projectPath = (id: string) => `/projects/${encodeURIComponent(id)}`;

export const projectSectionPath = (id: string, section: ProjectSection) =>
  `${projectPath(id)}/${section.segment}`;
