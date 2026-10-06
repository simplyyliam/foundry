import { ClipboardPaste, LoaderCircle, Pencil } from "lucide-react";
import { useEffect, useRef, useState, type ClipboardEvent } from "react";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverTitle, PopoverTrigger } from "@/components/ui/popover";
import type { AvatarChoice } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton, shortcut } from "@/shared";
import { useBuiltinAvatars } from "../hooks/useBuiltinAvatars";
import { DEFAULT_AVATAR } from "../lib/avatar";
import { ErrorLine } from "./ErrorLine";

interface AvatarPopoverProps {
  name: string;
  src: string;
  /** Resolves with null on success, or the backend's message. */
  onChoose: (choice: AvatarChoice) => Promise<string | null>;
}

/** The agent's avatar; clicking it opens "Change avatar". */
export function AvatarPopover({ name, src, onChoose }: AvatarPopoverProps) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <PressButton
            variant="ghost"
            pressScale={0.94}
            aria-label={`Change avatar for ${name}`}
            className="group/avatar-button relative size-10 shrink-0 rounded-full p-0 hover:bg-transparent"
          />
        }
      >
        <Avatar size="lg">
          <AvatarImage src={src} alt="" />
          <AvatarFallback>
            <img src={DEFAULT_AVATAR} alt="" className="size-full rounded-full object-cover" />
          </AvatarFallback>
        </Avatar>
        <span
          aria-hidden
          className="absolute -right-0.5 -bottom-0.5 flex size-4.5 items-center justify-center rounded-full bg-popover opacity-0 shadow-sm ring-1 ring-foreground/10 transition-opacity group-hover/avatar-button:opacity-100 group-focus-visible/avatar-button:opacity-100"
        >
          <Pencil className="size-2.5" />
        </span>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-72 gap-3 p-3">
        {/* Mounts on open: built-ins are read then, and the paste target can take focus. */}
        <AvatarChooser
          name={name}
          onChoose={async (choice) => {
            const message = await onChoose(choice);
            if (message === null) setOpen(false);
            return message;
          }}
        />
      </PopoverContent>
    </Popover>
  );
}

function AvatarChooser({ name, onChoose }: Pick<AvatarPopoverProps, "name" | "onChoose">) {
  const builtins = useBuiltinAvatars();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pasteRef = useRef<HTMLDivElement>(null);

  // Focus the paste target so Ctrl+V works straight away (after the popup has positioned).
  useEffect(() => {
    const frame = requestAnimationFrame(() => pasteRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  const choose = async (choice: AvatarChoice) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    const message = await onChoose(choice);
    setBusy(false);
    setError(message);
  };

  const onPaste = async (event: ClipboardEvent) => {
    const file = Array.from(event.clipboardData.files).find((item) => item.type.startsWith("image/"));
    if (!file) {
      // Text pasted into the filter field etc. isn't ours to handle.
      if ((event.target as HTMLElement).closest("input, textarea")) return;
      event.preventDefault();
      setError("The clipboard has no image. Copy an image, then paste it here.");
      return;
    }
    event.preventDefault();
    const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
    await choose({ builtinId: null, bytes });
  };

  return (
    <div className="flex flex-col gap-3" onPaste={(event) => void onPaste(event)}>
      <PopoverHeader>
        <PopoverTitle className="text-[13px]">Change avatar</PopoverTitle>
        <PopoverDescription className="text-[12.5px] leading-snug">For {name}.</PopoverDescription>
      </PopoverHeader>

      {builtins.status === "error" && <ErrorLine message={builtins.message} />}
      {builtins.status === "ready" && builtins.ids.length > 0 && (
        <div role="group" aria-label="Built-in avatars" className="flex flex-wrap gap-1.5">
          {builtins.ids.map((id) => (
            <PressButton
              key={id}
              size="xs"
              variant="outline"
              className="rounded-md"
              disabled={busy}
              onClick={() => void choose({ builtinId: id, bytes: null })}
            >
              {id}
            </PressButton>
          ))}
        </div>
      )}

      <div className="flex flex-col gap-1.5">
        <p className="text-[12px] font-medium text-muted-foreground">Upload image</p>
        <div
          ref={pasteRef}
          tabIndex={0}
          role="group"
          aria-label={`Paste an image (${shortcut("V")})`}
          aria-busy={busy || undefined}
          className={cn(
            "flex flex-col items-center gap-1.5 rounded-lg border border-dashed border-border px-3 py-5 text-center text-[12.5px] text-muted-foreground outline-none",
            "focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30",
          )}
        >
          {busy ? (
            <LoaderCircle aria-label="Uploading" className="size-4 animate-spin" />
          ) : (
            <ClipboardPaste aria-hidden className="size-4" />
          )}
          <span>
            Copy an image, then press <kbd className="font-sans font-medium text-foreground">{shortcut("V")}</kbd> here.
          </span>
          <span className="text-[11.5px]">PNG, JPEG or WebP.</span>
        </div>
      </div>

      {error && <ErrorLine message={error} />}
    </div>
  );
}
