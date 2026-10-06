import { Check, Copy, FileText } from "lucide-react";
import { useEffect, useState } from "react";
import { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverTitle, PopoverTrigger } from "@/components/ui/popover";
import { PressButton } from "@/shared";

const EXAMPLE = `---
title: Release Manager
description: Cuts releases and keeps the changelog honest.
writes: [CHANGELOG.md, "release/**"]
protected:
  - src/**
---
You are the Release Manager. You ship releases.

You never cut a release without updating CHANGELOG.md,
and you never edit application code.
`;

/** "Format": what a dropped role file looks like, as a selectable (and copyable) example. */
export function RoleFormatPopover() {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1500);
    return () => clearTimeout(timer);
  }, [copied]);

  const copy = () => {
    navigator.clipboard.writeText(EXAMPLE).then(
      () => setCopied(true),
      // The webview can refuse clipboard access; the text is still selectable.
      (error: unknown) => console.warn("Couldn't copy the example", error),
    );
  };

  return (
    <Popover>
      <PopoverTrigger render={<PressButton size="sm" variant="ghost" className="h-8 rounded-md text-muted-foreground" />}>
        <FileText data-icon="inline-start" />
        Format
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[22rem] gap-3">
        <PopoverHeader>
          <PopoverTitle className="text-[13px]">Role file format</PopoverTitle>
          <PopoverDescription className="text-[12.5px] leading-snug">
            One role per .md file. Every key is optional, and so is the whole --- block. Without a title, the first heading or
            the file name is used; without a description, the first paragraph. Everything after the block is the role's
            skill.
          </PopoverDescription>
        </PopoverHeader>
        <div className="relative">
          <pre className="overflow-x-auto rounded-lg bg-muted px-3 py-2.5 font-mono text-[11.5px] leading-relaxed select-text">
            {EXAMPLE}
          </pre>
          <PressButton
            pressScale={0.9}
            variant="ghost"
            size="icon-xs"
            aria-label={copied ? "Copied" : "Copy example"}
            onClick={copy}
            className="absolute top-1.5 right-1.5 rounded-md bg-muted"
          >
            {copied ? <Check aria-hidden /> : <Copy aria-hidden />}
          </PressButton>
        </div>
      </PopoverContent>
    </Popover>
  );
}
