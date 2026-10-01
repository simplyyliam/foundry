import { Plus } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Dialog, DialogContent, DialogTrigger } from "@/components/ui/dialog";
import { PROJECT_SECTIONS, projectSectionPath } from "@/features/projects";
import { SidebarGroupAction, usePressScale } from "@/shared";
import { AddProjectFlow } from "./AddProjectFlow";

/** The Projects "+" button and the dialog it opens. */
export function AddProjectDialog() {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const pressRef = usePressScale<HTMLButtonElement>({ scale: 0.92 });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger ref={pressRef} render={<SidebarGroupAction className="relative top-auto right-auto" />}>
        <Plus /> <span className="sr-only">Add project</span>
      </DialogTrigger>
      {/* Edge-to-edge search layout; Escape or clicking outside closes it. */}
      <DialogContent showCloseButton={false} className="gap-0 overflow-hidden p-0">
        <AddProjectFlow
          onDone={(project) => {
            setOpen(false);
            // A new project opens on its first page (the idea doc).
            navigate(projectSectionPath(project.id, PROJECT_SECTIONS[0]));
          }}
        />
      </DialogContent>
    </Dialog>
  );
}
