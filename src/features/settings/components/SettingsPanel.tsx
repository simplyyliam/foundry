import { LoaderCircle, X } from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useState } from "react";
import { DialogClose, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type { Settings } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { reloadSettings, useSettingsState } from "../hooks/settingsStore";
import { isSectionId, SECTIONS, type SectionId } from "../lib/sections";
import { AboutSection } from "./sections/AboutSection";
import { AgentsSection } from "./sections/AgentsSection";
import { AppearanceSection } from "./sections/AppearanceSection";
import { GeneralSection } from "./sections/GeneralSection";
import { ModelsSection } from "./sections/ModelsSection";
import { NotificationsSection } from "./sections/NotificationsSection";
import { SettingsNav } from "./SettingsNav";

/** The section shown last, for the next open in this session. */
let lastSection: SectionId = "general";

interface SettingsPanelProps {
  /** Section to open on; anything unknown (or null) reopens the last one. */
  initialSection?: string | null;
}

/**
 * The Settings dialog's body: sections on the left, the chosen one on the right. Every control
 * saves on its own. Mounted only while the dialog is open.
 */
export function SettingsPanel({ initialSection = null }: SettingsPanelProps) {
  const [active, setActive] = useState<SectionId>(isSectionId(initialSection) ? initialSection : lastSection);
  const state = useSettingsState();

  // Pick up anything changed since the last read (queued behind any save still running).
  useEffect(() => {
    void reloadSettings();
  }, []);

  const select = (id: SectionId) => {
    lastSection = id;
    setActive(id);
  };

  const meta = SECTIONS.find((section) => section.id === active) ?? SECTIONS[0];

  return (
    <div className="flex h-full min-h-0 w-full">
      <DialogTitle className="sr-only">Settings</DialogTitle>
      <DialogDescription className="sr-only">Changes save as you make them.</DialogDescription>
      <SettingsNav active={active} onSelect={select} />

      <div className="relative flex min-w-0 flex-1 flex-col">
        <CloseButton />
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-8 pt-6 pb-8">
          <motion.div
            key={active}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.16, ease: [0.32, 0.72, 0, 1] }}
            className="mx-auto flex w-full max-w-2xl flex-col"
          >
            <h2 className="pr-10 text-[15px] font-semibold">{meta.label}</h2>
            {active === "about" ? (
              <AboutSection />
            ) : state.status === "loading" ? (
              <div className="flex justify-center py-10">
                <LoaderCircle aria-label="Loading settings" className="size-4 animate-spin text-muted-foreground" />
              </div>
            ) : state.status === "error" ? (
              <LoadError message={state.message} />
            ) : (
              <Section id={active} settings={state.settings} />
            )}
          </motion.div>
        </div>
      </div>
    </div>
  );
}

function Section({ id, settings }: { id: Exclude<SectionId, "about">; settings: Settings }) {
  switch (id) {
    case "general":
      return <GeneralSection settings={settings} />;
    case "appearance":
      return <AppearanceSection settings={settings} />;
    case "models":
      return <ModelsSection settings={settings} />;
    case "agents":
      return <AgentsSection settings={settings} />;
    case "notifications":
      return <NotificationsSection settings={settings} />;
  }
}

function LoadError({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-start gap-3 py-4">
      <p role="alert" className="text-[12px] text-destructive">
        {message}
      </p>
      <PressButton size="sm" variant="outline" className="rounded-md" onClick={() => void reloadSettings()}>
        Try again
      </PressButton>
    </div>
  );
}

/** Its own component so the press feedback attaches when the dialog mounts it. */
function CloseButton() {
  return (
    <DialogClose
      render={
        <PressButton
          variant="ghost"
          size="icon-sm"
          aria-label="Close settings"
          className="absolute top-4 right-4 z-10 size-7 rounded-md text-muted-foreground hover:text-foreground"
        />
      }
    >
      <X className="size-4" />
    </DialogClose>
  );
}
