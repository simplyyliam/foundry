import { FolderOpen } from "lucide-react";
import { basename } from "@/features/projects";
import { usePressScale } from "@/shared";

interface DirectoryButtonProps {
  /** The folder projects live in, or null before one is chosen. */
  dir: string | null;
  onClick: () => void;
}

/** Sits beside the search field: shows and changes the projects folder. */
export function DirectoryButton({ dir, onClick }: DirectoryButtonProps) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.95 });

  return (
    <button
      ref={ref}
      type="button"
      onClick={onClick}
      title={dir ?? undefined}
      aria-label={dir ? `Projects folder: ${dir}. Change folder` : "Choose projects folder"}
      className="flex h-8 max-w-44 shrink-0 items-center gap-1.5 rounded-lg px-2.5 text-[12.5px] font-medium text-muted-foreground outline-none transition-colors duration-150 hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/40"
    >
      <FolderOpen aria-hidden className="size-4 shrink-0" strokeWidth={1.75} />
      <span className="truncate">{dir ? basename(dir) : "Choose folder"}</span>
    </button>
  );
}
