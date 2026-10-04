import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { usePressScale } from "@/shared";
import { answerDiscard, type GenerationState } from "../hooks/useGeneration";

/**
 * Asks before discarding confirmed requirements: before generating, and again if loading
 * would discard more than was agreed (or the backend answers `confirmed_requirements`).
 */
export function DiscardDialog({ projectId, state }: { projectId: string; state: GenerationState }) {
  const cancelRef = usePressScale<HTMLButtonElement>();
  const confirmRef = usePressScale<HTMLButtonElement>();
  const prompt = state.phase === "confirming" ? state.prompt : null;
  const count = prompt?.count ?? 0;

  return (
    <Dialog
      open={prompt !== null}
      onOpenChange={(open) => {
        if (!open) answerDiscard(projectId, false);
      }}
    >
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>
            Regenerating discards {count} confirmed {count === 1 ? "requirement" : "requirements"}
          </DialogTitle>
          <DialogDescription>
            {prompt?.when === "before-load"
              ? "The new roadmap is ready. Loading it replaces the plan, and every requirement goes back to proposed."
              : "The new roadmap replaces the plan, and every requirement goes back to proposed."}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button ref={cancelRef} variant="ghost" onClick={() => answerDiscard(projectId, false)}>
            Cancel
          </Button>
          <Button ref={confirmRef} variant="destructive" onClick={() => answerDiscard(projectId, true)}>
            {prompt?.when === "before-load" ? "Discard and load" : "Discard and regenerate"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
