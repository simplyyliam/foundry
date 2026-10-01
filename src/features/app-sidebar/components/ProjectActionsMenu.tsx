import { MoreHorizontal, type LucideIcon } from "lucide-react";
import { useRef } from "react";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { SidebarMenuAction, usePressScale } from "@/shared";

export interface ProjectAction {
  id: string;
  label: string;
  icon: LucideIcon;
  destructive?: boolean;
  /** The action moves focus itself (e.g. into an input): don't return it to the trigger. */
  keepsFocus?: boolean;
  onSelect: () => void;
}

interface ProjectActionsMenuProps {
  projectName: string;
  actions: readonly ProjectAction[];
}

/** The "more" button on a project row: shown on hover or focus, opens the project's actions. */
export function ProjectActionsMenu({ projectName, actions }: ProjectActionsMenuProps) {
  const pressRef = usePressScale<HTMLButtonElement>({ scale: 0.9 });
  const keepFocus = useRef(false);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        ref={pressRef}
        render={<SidebarMenuAction showOnHover className="top-1.5! data-popup-open:opacity-100" />}
        aria-label={`More actions for ${projectName}`}
      >
        <MoreHorizontal />
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="start"
        className="w-auto min-w-40"
        finalFocus={() => {
          const restore = !keepFocus.current;
          keepFocus.current = false;
          return restore;
        }}
      >
        {actions.map((action) => (
          <DropdownMenuItem
            key={action.id}
            variant={action.destructive ? "destructive" : "default"}
            className="text-[13px]"
            onClick={() => {
              keepFocus.current = action.keepsFocus ?? false;
              action.onSelect();
            }}
          >
            <action.icon strokeWidth={1.75} />
            {action.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
