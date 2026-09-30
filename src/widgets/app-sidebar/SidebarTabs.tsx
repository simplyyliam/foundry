import { NavLink, useMatch } from "react-router-dom";
import type { Tab } from "@/routing";
import { cn } from "@/lib/utils";
import {
  Icon,
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/shared";

function SidebarTab({ tab }: { tab: Tab }) {
  // `end: false` keeps the tab active on nested routes (/home/anything).
  const isActive = useMatch({ path: tab.path, end: false }) !== null;

  return (
    <SidebarMenuItem>
      <SidebarMenuButton
        render={<NavLink to={tab.path} />}
        isActive={isActive}
        tooltip={tab.label}
        onPointerEnter={tab.preload}
        onFocus={tab.preload}
        className="p-2.5! h-fit!"
      >
        {tab.icon && (
          <span
            className={cn(
              "flex size-7 shrink-0 items-center justify-center rounded-sm bg-accent transition-colors",
              // Deeper than accent, which matches the row's own hover/active background.
              "group-hover/menu-button:bg-[color-mix(in_oklch,var(--accent),var(--foreground)_7%)]",
              "group-data-active/menu-button:bg-[color-mix(in_oklch,var(--accent),var(--foreground)_7%)]",
            )}
          >
            <Icon icon={tab.icon} />
          </span>
        )}
        <span>{tab.label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  );
}

/** Top-level navigation, one item per folder in pages/app/tabs. */
export function SidebarTabs({ tabs }: { tabs: readonly Tab[] }) {
  if (tabs.length === 0) return null;

  return (
    <SidebarGroup>
      <SidebarGroupContent>
        <SidebarMenu>
          {tabs.map((tab) => (
            <SidebarTab key={tab.id} tab={tab} />
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}
