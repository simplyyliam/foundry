import { Check, Copy, LoaderCircle, Terminal } from "lucide-react";
import { useState } from "react";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { Plan, PromptFile, TeamAgent } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useCopy } from "../hooks/useCopy";
import { useLaunchMessage, usePromptText } from "../hooks/usePromptFile";
import { ErrorLine } from "./Notices";
import { AgentName, SliceName } from "./PromptList";

interface PromptSheetProps {
  projectId: string;
  /** The open file's path, or null when closed. */
  path: string | null;
  /** The listed file for that path, if the list has it (its kind and agent). */
  file: PromptFile | null;
  plan: Plan | null;
  agents: TeamAgent[] | null;
  onClose: () => void;
}

/** One prompt file's text, read-only, with Copy (and Copy launch message for task prompts). */
export function PromptSheet({ path, onClose, ...rest }: PromptSheetProps) {
  // Keep showing the last file while the sheet animates out.
  const [shown, setShown] = useState(path);
  if (path !== null && path !== shown) setShown(path);

  return (
    <Sheet open={path !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full gap-0 data-[side=right]:sm:max-w-xl">
        {shown && <PromptDetails key={shown} path={shown} {...rest} />}
      </SheetContent>
    </Sheet>
  );
}

function PromptDetails({ projectId, path, file, plan, agents }: Omit<PromptSheetProps, "onClose" | "path"> & { path: string }) {
  const text = usePromptText(projectId, path);
  const launchMessage = useLaunchMessage(projectId);
  const [copyState, copy] = useCopy();
  const [launchCopyState, copyLaunch] = useCopy();
  const [launch, setLaunch] = useState<
    { status: "idle" } | { status: "busy" } | { status: "ready"; message: string } | { status: "error"; message: string }
  >({ status: "idle" });

  const isTask = file?.kind === "task" && file.agentId !== null;

  const onCopyLaunch = async () => {
    if (!file?.agentId) return;
    setLaunch({ status: "busy" });
    const outcome = await launchMessage(file.agentId, path);
    if (!outcome.ok) {
      setLaunch({ status: "error", message: outcome.error.message });
      return;
    }
    setLaunch({ status: "ready", message: outcome.data });
    await copyLaunch(outcome.data);
  };

  return (
    <>
      <SheetHeader className="gap-1 pr-14 pb-4">
        <SheetTitle className="truncate text-[15px]">
          {file?.kind === "handoff" ? "Hand-off" : file ? <SliceName sliceId={file.sliceId} plan={plan} /> : path}
        </SheetTitle>
        <SheetDescription className="flex min-w-0 flex-col gap-0.5 text-[12.5px]">
          {isTask && (
            <span className="truncate">
              <AgentName agentId={file.agentId} agents={agents} />
            </span>
          )}
          <span className="truncate font-mono text-[11.5px]">{path}</span>
        </SheetDescription>
        <div className="mt-2 flex flex-wrap gap-2">
          <PressButton
            size="sm"
            variant="secondary"
            disabled={text.status !== "ready"}
            onClick={() => text.status === "ready" && void copy(text.text)}
          >
            {copyState === "copied" ? <Check data-icon="inline-start" /> : <Copy data-icon="inline-start" />}
            {copyState === "copied" ? "Copied" : "Copy"}
          </PressButton>
          {isTask && (
            <PressButton size="sm" variant="secondary" disabled={launch.status === "busy"} onClick={() => void onCopyLaunch()}>
              {launch.status === "busy" ? (
                <LoaderCircle data-icon="inline-start" className="animate-spin" />
              ) : launchCopyState === "copied" ? (
                <Check data-icon="inline-start" />
              ) : (
                <Terminal data-icon="inline-start" />
              )}
              Copy launch message
            </PressButton>
          )}
        </div>
        {copyState === "failed" && (
          <p role="alert" className="text-[12.5px] text-muted-foreground">
            Couldn't copy to the clipboard. Select the text below instead.
          </p>
        )}
        {launch.status === "error" && <ErrorLine message={launch.message} className="mt-1" />}
        {launch.status === "ready" && (
          <div role="status" className="mt-1 flex flex-col gap-1 rounded-lg bg-muted/60 px-3 py-2 ring-1 ring-foreground/5">
            <p className="text-[12px] text-muted-foreground">
              {launchCopyState === "failed" ? "Couldn't copy to the clipboard. Select it here:" : "Copied launch message:"}
            </p>
            <p className="font-mono text-[12px] leading-snug break-words text-foreground select-text">{launch.message}</p>
          </div>
        )}
      </SheetHeader>

      <div className="min-h-0 flex-1 border-t border-border/60">
        {text.status === "loading" && (
          <div className="flex justify-center py-10">
            <LoaderCircle aria-label="Loading the prompt" className="size-4 animate-spin text-muted-foreground" />
          </div>
        )}
        {text.status === "error" && <ErrorLine message={text.message} className="m-6" />}
        {text.status === "ready" && (
          <pre
            tabIndex={0}
            aria-label="Prompt text"
            className="h-full overflow-auto bg-muted/30 px-6 py-4 font-mono text-[12px] leading-relaxed whitespace-pre-wrap break-words outline-none select-text focus-visible:ring-2 focus-visible:ring-ring/40 focus-visible:ring-inset"
          >
            {text.text}
          </pre>
        )}
      </div>
    </>
  );
}
