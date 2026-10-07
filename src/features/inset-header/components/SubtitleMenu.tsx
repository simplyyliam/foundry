import { ChevronDown } from "lucide-react";
import { useNavigate } from "react-router-dom";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { usePressScale, type CrumbMenu } from "@/shared";

/** A crumb's muted subtitle as a dropdown trigger; picking an item navigates to it. */
export function SubtitleMenu({ subtitle, menu }: { subtitle: string; menu: CrumbMenu }) {
  const ref = usePressScale<HTMLButtonElement>();
  const navigate = useNavigate();

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        ref={ref}
        aria-label={`${subtitle}. ${menu.label}`}
        className="-mx-1.5 flex min-w-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 aria-expanded:bg-muted aria-expanded:text-foreground"
      >
        <span className="truncate">{subtitle}</span>
        <ChevronDown aria-hidden className="size-3.5 shrink-0" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-h-80 w-56 min-w-56">
        <DropdownMenuRadioGroup
          value={menu.current ?? null}
          onValueChange={(to: string) => {
            if (to !== menu.current) navigate(to);
          }}
        >
          {menu.items.map((item) => (
            <DropdownMenuRadioItem key={item.to} value={item.to}>
              <span className="truncate">{item.title}</span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
