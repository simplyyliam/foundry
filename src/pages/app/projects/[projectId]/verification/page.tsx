import { ProjectRoute, type Project } from "@/features/projects";
import { usePageHeader } from "@/shared";
import { Verification } from "@/widgets";

function VerificationPage({ project }: { project: Project }) {
  usePageHeader([{ title: project.name }, { title: "Verification" }]);
  return (
    <div className="h-full w-full overflow-y-auto bg-background">
      <div className="mx-auto max-w-3xl px-6 pt-6 pb-16">
        <Verification projectId={project.id} />
      </div>
    </div>
  );
}

/** /projects/:projectId/verification */
export default function VerificationRoute() {
  return <ProjectRoute>{(project) => <VerificationPage project={project} />}</ProjectRoute>;
}
