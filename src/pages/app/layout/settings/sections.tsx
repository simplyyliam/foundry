import type { ComponentType } from "react";
import { Bell, Bot, BookUser, Info, Library, Palette, Settings2, Sparkles, type LucideIcon } from "lucide-react";
import {
  AboutSettings,
  AgentsSettings,
  AppearanceSettings,
  DesignLibrary,
  GeneralSettings,
  ModelsSettings,
  NotificationsSettings,
  RoleHandbook,
} from "@/widgets";

export interface SettingsSection {
  id: string;
  /** Nav label and the heading above the section. */
  title: string;
  icon: LucideIcon;
  /** The section's body, a widget. Mounted only while the section is shown. */
  component: ComponentType;
  /** What the nav search also matches (lowercase), e.g. the setting names inside. */
  keywords: readonly string[];
}

/**
 * The Settings dialog's sections, in nav order. A new section is one entry here: the frame
 * (`SettingsFrame`) only composes, so sections from different widgets sit side by side.
 */
export const SETTINGS_SECTIONS: readonly SettingsSection[] = [
  {
    id: "general",
    title: "General",
    icon: Settings2,
    component: GeneralSettings,
    keywords: ["projects folder", "max concurrent agents", "claude path", "agents.md template"],
  },
  {
    id: "appearance",
    title: "Appearance",
    icon: Palette,
    component: AppearanceSettings,
    keywords: ["theme", "dark", "light", "reduce motion", "animation"],
  },
  {
    id: "models",
    title: "AI & Models",
    icon: Sparkles,
    component: ModelsSettings,
    keywords: ["default model", "roadmap generation timeout", "claude code status", "signed in", "login"],
  },
  {
    id: "agents",
    title: "Agents & Automation",
    icon: Bot,
    component: AgentsSettings,
    keywords: ["hooks", "verify mode"],
  },
  {
    id: "role-handbook",
    title: "Role Handbook",
    icon: BookUser,
    component: RoleHandbook,
    keywords: ["roles", "product lead", "skill", "write patterns", "protected patterns", "new role"],
  },
  {
    id: "design-library",
    title: "Design Library",
    icon: Library,
    component: DesignLibrary,
    keywords: ["design guidelines", "markdown", ".md files", "family", "tags"],
  },
  {
    id: "notifications",
    title: "Notifications",
    icon: Bell,
    component: NotificationsSettings,
    keywords: ["desktop notifications", "severity"],
  },
  { id: "about", title: "About", icon: Info, component: AboutSettings, keywords: ["version", "os"] },
];

export function findSection(id: string | null): SettingsSection | undefined {
  return SETTINGS_SECTIONS.find((section) => section.id === id);
}

export function matchSections(query: string): SettingsSection[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [...SETTINGS_SECTIONS];
  return SETTINGS_SECTIONS.filter(
    (section) =>
      section.title.toLowerCase().includes(needle) || section.keywords.some((keyword) => keyword.includes(needle)),
  );
}
