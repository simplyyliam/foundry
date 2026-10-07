import { ProjectRoute, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Prompts } from "@/widgets";

function PromptsPage({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Prompts" }]);
  return (
    <div className="h-full w-full overflow-y-auto bg-background">
      <div className="mx-auto max-w-3xl px-6 pt-6 pb-16">
        <Prompts projectId={project.id} />
      </div>
    </div>
  );
}

/** /projects/:projectId/prompts */
export default function PromptsRoute() {
  return <ProjectRoute>{(project) => <PromptsPage project={project} />}</ProjectRoute>;
}
