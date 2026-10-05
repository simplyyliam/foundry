import { ChevronRight, Folder, FolderGit2 } from "lucide-react";
import type { FolderEntry } from "@/lib/bindings";
import { usePressScale } from "@/shared";

interface FolderRowProps {
  folder: FolderEntry;
  onOpen: (path: string) => void;
}

export function FolderRow({ folder, onOpen }: FolderRowProps) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.985 });
  const Icon = folder.isGitRepo ? FolderGit2 : Folder;

  return (
    <li>
      <button
        ref={ref}
        type="button"
        data-folder-row
        onClick={() => onOpen(folder.path)}
        className="group/row flex h-8 w-full items-center gap-2.5 rounded-lg px-2.5 text-left text-[13px] outline-none transition-colors duration-100 hover:bg-accent focus-visible:bg-accent"
      >
        <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
        <span className="min-w-0 flex-1 truncate">{folder.name}</span>
        {folder.isGitRepo && (
          <span className="shrink-0 rounded-md bg-secondary px-1.5 py-px text-[11px] font-medium text-muted-foreground">
            git
          </span>
        )}
        <ChevronRight
          aria-hidden
          className="size-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity duration-100 group-hover/row:opacity-100 group-focus-visible/row:opacity-100"
        />
      </button>
    </li>
  );
}
