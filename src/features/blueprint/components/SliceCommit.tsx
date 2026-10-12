import { useState } from "react";
import type { SliceView } from "@/lib/bindings";
import { CommitSliceButton, CommitSliceDialog, CommitSlicePrompt, commitAvailability } from "@/shared";
import { useSliceCommit } from "../hooks/useSliceCommit";

interface SliceCommitProps {
  projectId: string;
  slice: SliceView;
  /** "Mark done" just succeeded: ask once whether to commit. */
  prompt: boolean;
  onPromptDone: () => void;
}

/** "Commit changes" for the slice sheet, and the one-time prompt after marking it done. */
export function SliceCommit({ projectId, slice, prompt, onPromptDone }: SliceCommitProps) {
  const controller = useSliceCommit(projectId, slice.id);
  const availability = commitAvailability(controller);
  const [open, setOpen] = useState(false);
  const openDialog = () => {
    onPromptDone();
    setOpen(true);
  };

  return (
    <div className="flex flex-col gap-2">
      {prompt && slice.status === "done" ? (
        <CommitSlicePrompt availability={availability} onOpen={openDialog} onDismiss={onPromptDone} />
      ) : (
        <CommitSliceButton availability={availability} onOpen={openDialog} size="xs" />
      )}
      <CommitSliceDialog open={open} onOpenChange={setOpen} sliceName={slice.name} controller={controller} />
    </div>
  );
}
