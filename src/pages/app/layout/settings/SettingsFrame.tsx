import { X } from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { PressButton, useSettingsDialog } from "@/shared";
import { findSection, SETTINGS_SECTIONS } from "./sections";
import { SettingsNav } from "./SettingsNav";

/** The section shown last, for the next open in this session. */
let lastSection = SETTINGS_SECTIONS[0].id;

/**
 * The Settings dialog: the sections from `SETTINGS_SECTIONS` on the left, the chosen one on
 * the right. Opened through `useSettingsDialog()` (the sidebar's gear). The frame only
 * composes: each section is a widget, so it can mix app settings with the libraries.
 */
export function SettingsFrame() {
  const { open, section, setOpen } = useSettingsDialog();

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(40rem,calc(100%-6rem))] w-[min(56rem,calc(100%-6rem))] max-w-none gap-0 overflow-hidden p-0 sm:max-w-none"
      >
        <Body initialSection={section} />
      </DialogContent>
    </Dialog>
  );
}

/** Mounts with the dialog. An unknown (or null) section reopens the last one. */
function Body({ initialSection }: { initialSection: string | null }) {
  const [active, setActive] = useState(() => findSection(initialSection)?.id ?? lastSection);
  const current = findSection(active) ?? SETTINGS_SECTIONS[0];
  const Section = current.component;

  const select = (id: string) => {
    lastSection = id;
    setActive(id);
  };

  return (
    <div className="flex h-full min-h-0 w-full">
      <DialogTitle className="sr-only">Settings</DialogTitle>
      <DialogDescription className="sr-only">Changes save as you make them.</DialogDescription>
      <SettingsNav active={current.id} onSelect={select} />

      <div className="relative flex min-w-0 flex-1 flex-col">
        <CloseButton />
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-8 pt-6 pb-8">
          <motion.div
            key={current.id}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.16, ease: [0.32, 0.72, 0, 1] }}
            className="mx-auto flex w-full max-w-2xl flex-col"
          >
            <h2 className="pr-10 text-[15px] font-semibold">{current.title}</h2>
            <Section />
          </motion.div>
        </div>
      </div>
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
