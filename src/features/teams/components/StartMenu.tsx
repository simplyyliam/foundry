import { ChevronDown, FileText, Play } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import type { PromptFile } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import type { StartChoices } from "../hooks/useStartChoices";
import { formatAge, formatFull } from "../lib/age";

/** What a session starts on: nothing (the agent waits for you), or a task prompt. */
export type StartChoice = { kind: "none" } | { kind: "task"; file: PromptFile; sliceName: string };

interface StartMenuProps {
  projectId: string;
  agentId: string;
  label: string;
  choices: StartChoices;
  onPick: (choice: StartChoice) => void;
  /** Tint-aware button colours (the card's selected state). */
  className?: string;
}

function ChoiceButton({ title, detail, tooltip, onClick }: { title: string; detail?: string; tooltip?: string; onClick: () => void }) {
  return (
    <PressButton
      variant="ghost"
      pressScale={0.98}
      title={tooltip}
      onClick={onClick}
      className="h-auto w-full justify-start gap-2.5 rounded-lg px-2.5 py-2 text-left font-normal"
    >
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate text-[13px] text-foreground">{title}</span>
        {detail && <span className="truncate text-[11.5px] text-muted-foreground">{detail}</span>}
      </span>
    </PressButton>
  );
}

/**
 * The Start button and its menu: "Start without a task" first, then this agent's task prompts
 * (slice name and age). The prompt files are re-read every time it opens.
 */
export function StartMenu({ projectId, agentId, label, choices, onPick, className }: StartMenuProps) {
  const { prompts, sliceNames, planError } = choices;
  const [open, setOpen] = useState(false);
  const pick = (choice: StartChoice) => {
    setOpen(false);
    onPick(choice);
  };
  const tasks =
    prompts.status === "ready" ? prompts.files.filter((file) => file.kind === "task" && file.agentId === agentId) : [];
  const nameOf = (sliceId: string | null) => (sliceId ? (sliceNames?.[sliceId] ?? sliceId) : "No slice");

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (next) choices.reload();
      }}
    >
      <PopoverTrigger
        render={
          <PressButton className={cn("gap-1.5 rounded-full pr-3 pl-3.5", className)}>
            <Play aria-hidden className="size-3.5 fill-current" />
            {label}
            <ChevronDown aria-hidden className="size-3.5 opacity-70" />
          </PressButton>
        }
      />
      <PopoverContent align="center" className="w-80 gap-1 p-1.5">
        <ChoiceButton
          title="Start without a task"
          detail="It reads its role, then waits for you to type"
          onClick={() => pick({ kind: "none" })}
        />
        <Separator className="my-1" />
        <p className="px-2.5 pt-1 pb-0.5 text-[11.5px] font-medium text-muted-foreground">Task prompts</p>
        {prompts.status === "loading" && <p className="px-2.5 py-2 text-[12.5px] text-muted-foreground">Loading…</p>}
        {prompts.status === "error" && (
          <p role="alert" className="px-2.5 py-2 text-[12.5px] text-destructive">
            {prompts.error.message}
          </p>
        )}
        {prompts.status === "ready" && tasks.length === 0 && (
          <p className="px-2.5 py-2 text-[12.5px] text-muted-foreground">
            <Link
              to={`/projects/${projectId}/prompts`}
              className="underline underline-offset-3 outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              Generate one on the Prompts page
            </Link>
          </p>
        )}
        {tasks.length > 0 && (
          <div className="flex max-h-64 flex-col overflow-y-auto">
            {tasks.map((file) => {
              const sliceName = nameOf(file.sliceId);
              return (
                <ChoiceButton
                  key={file.path}
                  title={sliceName}
                  detail={`Written ${formatAge(file.modifiedMs)}`}
                  tooltip={`${file.path}\n${formatFull(file.modifiedMs)}`}
                  onClick={() => pick({ kind: "task", file, sliceName })}
                />
              );
            })}
          </div>
        )}
        {planError && tasks.length > 0 && (
          <p role="alert" className="flex items-start gap-1.5 px-2.5 py-1.5 text-[11.5px] text-muted-foreground">
            <FileText aria-hidden className="mt-px size-3 shrink-0" />
            {planError.message}
          </p>
        )}
      </PopoverContent>
    </Popover>
  );
}
