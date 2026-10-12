import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { PROJECT_SECTIONS, projectSectionPath } from "@/features/projects";
import { usePressScale } from "@/shared";
import { useClaudeStatus } from "../hooks/useClaudeStatus";
import { useConflicts } from "../hooks/useConflicts";
import { useGenerateControl } from "../hooks/useGenerateControl";
import { cancelGeneration, useGeneration } from "../hooks/useGeneration";
import { useRoadmap } from "../hooks/useRoadmap";
import { Artifact, BodyText } from "./Artifact";
import { ConflictsPanel } from "./ConflictsPanel";
import { DiscardDialog } from "./DiscardDialog";
import { ErrorArtifact, GenerationProgress } from "./GenerationStatus";
import { NeedsALook } from "./NeedsALook";
import {
  BuildOrderSection,
  FeatureMapSection,
  GoalSection,
  LaterSection,
  OpenQuestionsSection,
  TargetUsersSection,
} from "./RoadmapSections";

interface RoadmapProps {
  projectId: string;
  /**
   * This instance shows the discard dialog. With the pop-out open only the pop-out does, so
   * the dialog nests inside it (and never shows twice).
   */
  ownsPrompts: boolean;
  /** Changes when a page asks to settle the conflicts: they scroll into view. */
  focusConflicts?: number;
  className?: string;
}

/**
 * The roadmap itself, used as-is by the panel and the pop-out: status boxes on top, then the
 * plan as the backend returned it. Narrow: one column; wide (`@2xl`): boxes in two columns.
 */
export function Roadmap({ projectId, ownsPrompts, focusConflicts, className }: RoadmapProps) {
  const data = useRoadmap(projectId);
  const generation = useGeneration(projectId);
  const claude = useClaudeStatus();
  const control = useGenerateControl(projectId);
  const conflicts = useConflicts(projectId);
  const generateRef = usePressScale<HTMLButtonElement>({ disabled: !control.canGenerate });
  const result = data.result;

  return (
    <div className={cn("@container flex flex-col gap-5", className)}>
      {generation.phase !== "idle" && generation.phase !== "confirming" && (
        <GenerationProgress state={generation} onCancel={() => void cancelGeneration(projectId)} />
      )}
      <ErrorArtifact errors={generation.errors} />
      {claude.problem && (
        <Artifact>
          <p role="alert" className="text-[13px] leading-snug text-foreground/85">
            {claude.problem.message}
          </p>
        </Artifact>
      )}
      {data.error && <ErrorArtifact errors={[data.error]} />}
      {result && <NeedsALook result={result} />}
      <ConflictsPanel controller={conflicts} focusToken={focusConflicts} />

      {data.status === "loading" ? (
        <div className="flex flex-col gap-2" aria-label="Loading roadmap">
          <Skeleton className="h-3.5 w-16" />
          <Skeleton className="h-3.5 w-full" />
          <Skeleton className="h-3.5 w-4/5" />
        </div>
      ) : result ? (
        <>
          <GoalSection goal={result.roadmap.goal} />
          <TargetUsersSection users={result.roadmap.targetUsers} />
          <FeatureMapSection families={result.roadmap.featureMap} />
          <BuildOrderSection roadmap={result.roadmap} />
          <LaterSection deferred={result.roadmap.deferred} />
          <OpenQuestionsSection questions={result.roadmap.openQuestions} />
        </>
      ) : data.briefEmpty ? (
        <div className="flex flex-col items-start gap-3 py-2">
          <BodyText>No brain dump yet. The roadmap is generated from the Idea page.</BodyText>
          <Button
            size="sm"
            variant="outline"
            render={<Link to={projectSectionPath(projectId, PROJECT_SECTIONS[0])} />}
          >
            Open Idea
          </Button>
        </div>
      ) : (
        data.status === "ready" && (
          <div className="flex flex-col items-start gap-3 py-2">
            <BodyText>No roadmap yet.</BodyText>
            <Button ref={generateRef} size="sm" disabled={!control.canGenerate} onClick={control.start}>
              Generate roadmap
            </Button>
          </div>
        )
      )}

      {ownsPrompts && <DiscardDialog projectId={projectId} state={generation} />}
    </div>
  );
}
