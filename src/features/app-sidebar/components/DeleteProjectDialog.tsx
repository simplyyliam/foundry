import { LoaderCircle } from "lucide-react";
import { useId, useState, type FormEvent } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { deleteRepository, type Project } from "@/features/projects";
import { PressButton } from "./add-project/PressButton";

interface DeleteProjectDialogProps {
  project: Project;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * Type-the-name confirmation before deleting a project's folder (Recycle Bin).
 * Calls `deleteRepository`, which is a PLACEHOLDER until the backend command exists.
 */
export function DeleteProjectDialog({ project, open, onOpenChange }: DeleteProjectDialogProps) {
  const [typed, setTyped] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputId = useId();
  const matches = typed === project.name;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!matches || deleting) return;
    setDeleting(true);
    setError(null);
    const outcome = await deleteRepository(project.id);
    setDeleting(false);
    if (outcome.ok) onOpenChange(false);
    else setError(outcome.error.message);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) {
          setTyped("");
          setError(null);
        }
      }}
    >
      <DialogContent className="sm:max-w-sm">
        <form onSubmit={submit} className="flex flex-col gap-5">
          <DialogHeader>
            <DialogTitle>Delete {project.name}?</DialogTitle>
            <DialogDescription>
              This moves the project folder to the Recycle Bin and removes it from Foundry.
              <span className="mt-2 block truncate font-mono text-[12px]" title={project.repoPath}>
                {project.repoPath}
              </span>
            </DialogDescription>
          </DialogHeader>
          <div className="flex flex-col gap-2">
            <label htmlFor={inputId} className="text-[13px] text-muted-foreground">
              Type <span className="font-medium text-foreground">{project.name}</span> to confirm
            </label>
            <Input
              id={inputId}
              value={typed}
              onChange={(event) => {
                setTyped(event.target.value);
                setError(null);
              }}
              autoComplete="off"
              spellCheck={false}
              autoFocus
            />
            {error && (
              <p role="alert" className="text-[12px] text-destructive">
                {error}
              </p>
            )}
          </div>
          <DialogFooter>
            <PressButton type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </PressButton>
            <PressButton type="submit" variant="destructive" disabled={!matches || deleting}>
              {deleting && <LoaderCircle className="animate-spin" />}
              Delete project
            </PressButton>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
