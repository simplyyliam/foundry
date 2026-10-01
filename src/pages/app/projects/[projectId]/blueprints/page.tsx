import { ProjectRoute, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";

function Blueprint({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Blueprint" }]);

  // Placeholder until the blueprints page is built.
  return (
    <div className="flex h-full w-full items-center justify-center bg-background text-sm text-muted-foreground">
      Blueprint
    </div>
  );
}

/** /projects/:projectId/blueprints */
export default function BlueprintsPage() {
  return <ProjectRoute>{(project) => <Blueprint project={project} />}</ProjectRoute>;
}
