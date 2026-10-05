import { Dialog, DialogContent } from "@/components/ui/dialog";
import { SettingsPanel } from "@/features/settings";
import { useSettingsDialog } from "@/shared";

/**
 * The Settings dialog. Opened through `useSettingsDialog()` (the sidebar's gear); the
 * provider lives in the app layout. Escape, the close button or clicking outside closes it.
 */
export function SettingsDialog() {
  const { open, section, setOpen } = useSettingsDialog();

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(40rem,calc(100%-6rem))] w-[min(56rem,calc(100%-6rem))] max-w-none gap-0 overflow-hidden p-0 sm:max-w-none"
      >
        <SettingsPanel initialSection={section} />
      </DialogContent>
    </Dialog>
  );
}
