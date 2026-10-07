import { useState } from "react";
import { usePromptPlan } from "../hooks/usePromptPlan";
import { usePrompts } from "../hooks/usePrompts";
import { usePromptTeam } from "../hooks/usePromptTeam";
import { HandoffCard } from "./HandoffCard";
import { PromptList } from "./PromptList";
import { PromptSheet } from "./PromptSheet";
import { RunByHand } from "./RunByHand";
import { TaskPromptCard } from "./TaskPromptCard";

/**
 * The Prompts page's content: generate the hand-off and task prompts, list them, read and copy
 * them. Everything is assembled by the backend from files on disk; nothing here uses Claude
 * or starts an agent.
 */
export function PromptsView({ projectId }: { projectId: string }) {
  const prompts = usePrompts(projectId);
  const plan = usePromptPlan(projectId);
  const team = usePromptTeam(projectId);
  const [openPath, setOpenPath] = useState<string | null>(null);

  const files = prompts.load.status === "ready" ? prompts.load.files : [];
  const loadedPlan = plan.load.status === "ready" ? plan.load.plan : null;
  const agents = team.load.status === "ready" ? team.load.agents : null;

  return (
    <div className="flex flex-col gap-8">
      <header className="flex flex-col gap-1">
        <p className="text-[13.5px] leading-snug text-pretty">
          Prompts are built from your confirmed plan and team. Generating them doesn't use your Claude usage.
        </p>
        <p className="text-[12.5px] leading-snug text-pretty text-muted-foreground">
          Prompts are snapshots. Regenerate after the plan changes.
        </p>
      </header>

      <div className="flex flex-col gap-4">
        <HandoffCard
          projectId={projectId}
          exists={files.some((file) => file.kind === "handoff")}
          generate={prompts.generateHandoff}
          onOpen={setOpenPath}
        />
        <TaskPromptCard
          projectId={projectId}
          plan={plan}
          team={team}
          generate={prompts.generateTaskPrompt}
          onOpen={setOpenPath}
        />
      </div>

      <PromptList load={prompts.load} reload={prompts.reload} plan={loadedPlan} agents={agents} onOpen={setOpenPath} />

      <RunByHand />

      <PromptSheet
        projectId={projectId}
        path={openPath}
        file={files.find((file) => file.path === openPath) ?? null}
        plan={loadedPlan}
        agents={agents}
        onClose={() => setOpenPath(null)}
      />
    </div>
  );
}
