import { Bell, Bot, Info, Palette, Settings2, Sparkles, type LucideIcon } from "lucide-react";

export type SectionId = "general" | "appearance" | "models" | "agents" | "notifications" | "about";

export interface SectionMeta {
  id: SectionId;
  label: string;
  icon: LucideIcon;
  /** Setting names in the section, so the search finds "theme" under Appearance. */
  keywords: readonly string[];
}

export const SECTIONS: readonly SectionMeta[] = [
  {
    id: "general",
    label: "General",
    icon: Settings2,
    keywords: ["projects folder", "max concurrent agents", "claude path", "agents.md template"],
  },
  { id: "appearance", label: "Appearance", icon: Palette, keywords: ["theme", "dark", "light", "reduce motion", "animation"] },
  {
    id: "models",
    label: "AI & Models",
    icon: Sparkles,
    keywords: ["default model", "roadmap generation timeout", "claude code status", "signed in", "login"],
  },
  { id: "agents", label: "Agents & Automation", icon: Bot, keywords: ["hooks", "verify mode"] },
  { id: "notifications", label: "Notifications", icon: Bell, keywords: ["desktop notifications", "severity"] },
  { id: "about", label: "About", icon: Info, keywords: ["version", "os"] },
];

export function isSectionId(value: string | null): value is SectionId {
  return SECTIONS.some((section) => section.id === value);
}

export function matchSections(query: string): SectionMeta[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [...SECTIONS];
  return SECTIONS.filter(
    (section) =>
      section.label.toLowerCase().includes(needle) || section.keywords.some((keyword) => keyword.includes(needle)),
  );
}
