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

/**
 * A crumb's subtitle followed by a pill holding the current item (e.g. "Working on" [Breeze ▾]).
 * Only the pill opens the menu; picking an item navigates to it.
 */
export function SubtitleMenu({ subtitle, menu }: { subtitle: string | undefined; menu: CrumbMenu }) {
  const ref = usePressScale<HTMLButtonElement>();
  const navigate = useNavigate();
  const current = menu.items.find((item) => item.to === menu.current);

  return (
    <span className="flex min-w-0 items-center gap-1.5">
      {subtitle && <span className="shrink-0 text-muted-foreground">{subtitle}</span>}
      <DropdownMenu>
        <DropdownMenuTrigger
          ref={ref}
          aria-label={`${current?.title ?? ""}. ${menu.label}`}
          className="flex min-w-0 items-center gap-1 rounded-[6.89px] bg-pill py-0.5 pr-1.5 pl-2 text-foreground outline-none transition-colors hover:bg-pill-hover focus-visible:ring-2 focus-visible:ring-ring/50 aria-expanded:bg-pill-hover"
        >
          <span className="truncate">{current?.title ?? menu.label}</span>
          <ChevronDown aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
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
    </span>
  );
}
