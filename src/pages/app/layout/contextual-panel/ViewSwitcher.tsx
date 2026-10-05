import { ChevronDown } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { usePressScale } from "@/shared";
import { setProjectView, PROJECT_VIEW_IDS, type ProjectViewId } from "./projectView";

interface ViewSwitcherProps {
  current: ProjectViewId;
  titles: Record<ProjectViewId, string>;
}

/** The panel title on project routes: a dropdown between this project's views. */
export function ViewSwitcher({ current, titles }: ViewSwitcherProps) {
  const ref = usePressScale<HTMLButtonElement>();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        ref={ref}
        className="-mx-1.5 flex min-w-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-[14px] font-medium outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50 aria-expanded:bg-muted"
      >
        <h1 className="truncate">{titles[current]}</h1>
        <ChevronDown aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-44 min-w-44">
        <DropdownMenuRadioGroup value={current} onValueChange={(value: ProjectViewId) => setProjectView(value)}>
          {PROJECT_VIEW_IDS.map((id) => (
            <DropdownMenuRadioItem key={id} value={id}>
              {titles[id]}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
