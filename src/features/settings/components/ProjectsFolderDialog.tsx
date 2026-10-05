import { FolderBrowser } from "@/components/folder-picker";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { useFolderLocations } from "../hooks/useFolderLocations";

interface ProjectsFolderDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Where the picker starts; null starts at the home folder. */
  initialPath: string | null;
  onSelect: (path: string) => void;
}

/** The app's in-app folder picker, stacked over Settings. Escape closes just this one. */
export function ProjectsFolderDialog({ open, onOpenChange, initialPath, onSelect }: ProjectsFolderDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false} className="gap-0 overflow-hidden p-0 pt-4">
        <Body initialPath={initialPath} onSelect={onSelect} onCancel={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  );
}

/** Mounts with the dialog, so the locations are fetched (and the picker starts fresh) per open. */
function Body({ initialPath, onSelect, onCancel }: { initialPath: string | null; onSelect: (path: string) => void; onCancel: () => void }) {
  const locations = useFolderLocations();
  return (
    <>
      <DialogTitle className="px-5 pb-3 text-[13px] font-medium">Default projects folder</DialogTitle>
      <DialogDescription className="sr-only">Pick the folder new projects are created in.</DialogDescription>
      <FolderBrowser initialPath={initialPath} locations={locations} confirmVerb="Use" cancelLabel="Cancel" onSelect={onSelect} onCancel={onCancel} />
    </>
  );
}
