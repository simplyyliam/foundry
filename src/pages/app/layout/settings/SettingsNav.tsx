import { Search } from "lucide-react";
import { useMemo, useState, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import { matchSections, type SettingsSection } from "./sections";

interface SettingsNavProps {
  active: string;
  onSelect: (id: string) => void;
}

/** The left column: a search over section and setting names, then the sections. */
export function SettingsNav({ active, onSelect }: SettingsNavProps) {
  const [query, setQuery] = useState("");
  const sections = useMemo(() => matchSections(query), [query]);

  const onSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" && sections[0]) {
      event.preventDefault();
      onSelect(sections[0].id);
    } else if (event.key === "Escape" && query) {
      // First Escape clears the search; the next one closes the dialog.
      event.stopPropagation();
      setQuery("");
    }
  };

  return (
    <nav aria-label="Settings sections" className="flex w-56 shrink-0 flex-col gap-3 border-r border-border/60 p-3">
      <div className="relative">
        <Search aria-hidden className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" strokeWidth={1.75} />
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={onSearchKeyDown}
          placeholder="Search"
          aria-label="Search settings"
          autoComplete="off"
          spellCheck={false}
          className="h-8 rounded-md pl-8 text-[13px]"
        />
      </div>
      <div className="flex flex-col gap-0.5">
        <p className="px-2.5 pb-1 text-[11.5px] font-medium text-muted-foreground">Settings</p>
        {sections.length === 0 ? (
          <p className="px-2.5 py-2 text-[12.5px] text-muted-foreground">No settings match</p>
        ) : (
          sections.map((section) => (
            <NavItem key={section.id} section={section} active={section.id === active} onSelect={onSelect} />
          ))
        )}
      </div>
    </nav>
  );
}

function NavItem({ section, active, onSelect }: { section: SettingsSection; active: boolean; onSelect: (id: string) => void }) {
  const Icon = section.icon;

  return (
    <PressButton
      variant="ghost"
      pressScale={0.98}
      aria-current={active ? "page" : undefined}
      onClick={() => onSelect(section.id)}
      className={cn(
        "h-8 w-full justify-start gap-2.5 rounded-md px-2.5 text-[13px] font-normal text-muted-foreground hover:bg-accent/70 hover:text-foreground active:not-aria-[haspopup]:translate-y-0",
        active && "bg-accent font-medium text-foreground hover:bg-accent",
      )}
    >
      <Icon aria-hidden className="size-4 shrink-0" strokeWidth={1.75} />
      <span className="truncate">{section.title}</span>
    </PressButton>
  );
}
