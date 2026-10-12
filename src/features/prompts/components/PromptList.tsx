import { BookOpen, FileText, LoaderCircle, NotebookPen } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { Plan, PromptFile, TeamAgent } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { usePromptNote } from "../hooks/usePromptFile";
import type { PromptsLoad } from "../hooks/usePrompts";
import { formatModified, formatModifiedFull, formatSize, orderPrompts } from "../lib/prompts";
import { NoteEditor } from "./NoteEditor";
import { ErrorLine } from "./Notices";

interface PromptListProps {
  projectId: string;
  load: PromptsLoad;
  reload: () => void;
  /** Null until loaded: names then show as ids, never as "removed". */
  plan: Plan | null;
  agents: TeamAgent[] | null;
  onOpen: (path: string) => void;
}

/** Every prompt file: the hand-off first, then task prompts. Click opens it in the sheet. */
export function PromptList({ projectId, load, reload, plan, agents, onOpen }: PromptListProps) {
  return (
    <section aria-labelledby="prompts-list-title" className="flex flex-col gap-3">
      <h2 id="prompts-list-title" className="text-[14px] font-semibold">
        Prompts
      </h2>
      {load.status === "loading" && (
        <div className="flex justify-center py-8">
          <LoaderCircle aria-label="Loading the prompts" className="size-4 animate-spin text-muted-foreground" />
        </div>
      )}
      {load.status === "error" && (
        <ErrorLine message={load.error.message}>
          <PressButton size="xs" variant="outline" className="rounded-md bg-background" onClick={reload}>
            Try again
          </PressButton>
        </ErrorLine>
      )}
      {load.status === "ready" &&
        (load.files.length === 0 ? (
          <div className="flex flex-col items-center gap-1 rounded-xl border border-dashed border-border px-6 py-8 text-center">
            <FileText aria-hidden className="mb-1 size-5 text-muted-foreground/70" />
            <p className="text-[13px] font-medium">No prompts yet</p>
            <p className="max-w-xs text-[12.5px] leading-snug text-pretty text-muted-foreground">
              Generate the hand-off first, then a task prompt for each slice you want to start.
            </p>
          </div>
        ) : (
          <ul className="flex flex-col gap-1">
            {orderPrompts(load.files).map((file) => (
              <li key={file.path}>
                {file.kind === "task" ? (
                  <TaskPromptItem projectId={projectId} file={file} plan={plan} agents={agents} onOpen={() => onOpen(file.path)} />
                ) : (
                  <PromptRow file={file} plan={plan} agents={agents} onOpen={() => onOpen(file.path)} />
                )}
              </li>
            ))}
          </ul>
        ))}
    </section>
  );
}

interface RowProps {
  file: PromptFile;
  plan: Plan | null;
  agents: TeamAgent[] | null;
  onOpen: () => void;
}

/**
 * A task prompt's row with its note under it. The hand-off has none: agents read it from the
 * task prompt's pointer, never through a launch, so a note on it would never be sent.
 */
function TaskPromptItem({ projectId, ...row }: RowProps & { projectId: string }) {
  const note = usePromptNote(projectId, row.file.path);
  return (
    <div className="flex flex-col gap-0.5">
      <PromptRow {...row} hasNote={note.load.status === "ready" && note.load.note !== null} />
      <NoteEditor note={note} />
    </div>
  );
}

function PromptRow({ file, plan, agents, onOpen, hasNote = false }: RowProps & { hasNote?: boolean }) {
  const Icon = file.kind === "handoff" ? BookOpen : FileText;
  return (
    <PressButton
      variant="ghost"
      pressScale={0.99}
      onClick={onOpen}
      className="h-auto w-full justify-start gap-3 rounded-lg px-3 py-2.5 text-left font-normal ring-1 ring-foreground/6 hover:bg-accent/60"
    >
      <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[13px] font-medium">
            {file.kind === "handoff" ? "Hand-off" : <SliceName sliceId={file.sliceId} plan={plan} />}
          </span>
          {hasNote && (
            <Badge variant="secondary" className="h-4 shrink-0 gap-1 px-1.5 text-[10px]">
              <NotebookPen aria-hidden className="size-2.5" />
              Note added
            </Badge>
          )}
        </span>
        <span className="truncate text-[12px] text-muted-foreground">
          {file.kind === "handoff" ? "Every agent reads this" : <AgentName agentId={file.agentId} agents={agents} />}
        </span>
      </span>
      <span className="flex shrink-0 flex-col items-end gap-0.5 text-[12px] text-muted-foreground tabular-nums">
        <time dateTime={new Date(file.modifiedMs).toISOString()} title={formatModifiedFull(file.modifiedMs)}>
          {formatModified(file.modifiedMs)}
        </time>
        <span>{formatSize(file.size)}</span>
      </span>
    </PressButton>
  );
}

/** The slice's name from the plan, or its id with "slice removed" once the plan says it's gone. */
export function SliceName({ sliceId, plan }: { sliceId: string | null; plan: Plan | null }) {
  if (sliceId === null) return <>Task prompt</>;
  const slice = plan?.slices.find((candidate) => candidate.id === sliceId);
  if (slice) return <>{slice.name}</>;
  return (
    <>
      <span className="font-mono text-[12px]">{sliceId}</span>
      {plan && <span className="ml-1.5 font-normal text-muted-foreground">slice removed</span>}
    </>
  );
}

/** The agent's display name, or its id with "agent removed" once the team says it's gone. */
export function AgentName({ agentId, agents }: { agentId: string | null; agents: TeamAgent[] | null }) {
  if (agentId === null) return null;
  const agent = agents?.find((candidate) => candidate.id === agentId);
  if (agent) return <>{agent.displayName}</>;
  return (
    <>
      <span className="font-mono text-[11.5px]">{agentId}</span>
      {agents && <span className="ml-1.5">agent removed</span>}
    </>
  );
}
