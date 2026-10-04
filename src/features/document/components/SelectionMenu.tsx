import { useEffect, useRef, useState, type ComponentType, type KeyboardEvent, type RefObject } from "react";
import type { Editor } from "@tiptap/core";
import { NodeSelection } from "@tiptap/pm/state";
import { useEditorState } from "@tiptap/react";
import { BubbleMenu } from "@tiptap/react/menus";
import { Bold, Heading1, Heading2, Heading3, Italic, Link2, List, ListOrdered, type LucideProps } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { shortcut, usePressScale } from "@/shared";

interface MenuButtonProps {
  Icon: ComponentType<LucideProps>;
  label: string;
  shortcut?: string;
  active: boolean;
  onPress: () => void;
}

function MenuButton({ Icon, label, shortcut, active, onPress }: MenuButtonProps) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.92 });
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            ref={ref}
            variant="ghost"
            size="icon-sm"
            aria-label={label}
            aria-pressed={active}
            className="size-7 rounded-md text-muted-foreground aria-pressed:bg-muted aria-pressed:text-foreground"
            // Keep the editor focused and its selection intact.
            onMouseDown={(event) => event.preventDefault()}
            onClick={onPress}
          >
            <Icon className="size-3.5" strokeWidth={2.25} />
          </Button>
        }
      />
      <TooltipContent sideOffset={8}>
        {label}
        {shortcut && <span className="text-background/60">{shortcut}</span>}
      </TooltipContent>
    </Tooltip>
  );
}

function LinkField({ editor, onDone, inputRef }: { editor: Editor; onDone: () => void; inputRef: RefObject<HTMLInputElement | null> }) {
  const [href, setHref] = useState(() => (editor.getAttributes("link").href as string | undefined) ?? "");

  const apply = () => {
    const chain = editor.chain().focus().extendMarkRange("link");
    const value = href.trim();
    // "example.org" means the site, not a relative path.
    const absolute = value === "" || /^([a-z][a-z0-9+.-]*:|[#/])/i.test(value) ? value : `https://${value}`;
    if (absolute === "") chain.unsetLink().run();
    else chain.setLink({ href: absolute }).run();
    onDone();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      apply();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onDone();
      editor.commands.focus();
    }
  };

  return (
    <Input
      ref={inputRef}
      autoFocus
      value={href}
      onChange={(event) => setHref(event.target.value)}
      onKeyDown={onKeyDown}
      placeholder="Paste a link, then Enter"
      aria-label="Link address"
      className="h-7 w-60 rounded-md bg-transparent px-2 text-[13px] md:text-[13px] focus-visible:ring-0"
    />
  );
}

/**
 * The small menu over a text selection: bold, italic, link, headings 1–3 and lists. Hidden
 * in code blocks and when the editor is read-only. Ctrl/⌘+K opens the link field.
 */
export function SelectionMenu({ editor, scrollTarget }: { editor: Editor; scrollTarget: HTMLElement }) {
  const [mode, setMode] = useState<"format" | "link">("format");
  const linkInput = useRef<HTMLInputElement>(null);
  const active = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current.isActive("bold"),
      italic: current.isActive("italic"),
      link: current.isActive("link"),
      h1: current.isActive("heading", { level: 1 }),
      h2: current.isActive("heading", { level: 2 }),
      h3: current.isActive("heading", { level: 3 }),
      bullet: current.isActive("bulletList"),
      ordered: current.isActive("orderedList"),
    }),
  });

  // Keyboard path to the link field.
  useEffect(() => {
    const dom = editor.view.dom;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k" && !editor.state.selection.empty) {
        event.preventDefault();
        setMode("link");
      }
    };
    dom.addEventListener("keydown", onKeyDown);
    return () => dom.removeEventListener("keydown", onKeyDown);
  }, [editor]);

  const run = (command: (chain: ReturnType<Editor["chain"]>) => ReturnType<Editor["chain"]>) =>
    command(editor.chain().focus()).run();

  return (
    <BubbleMenu
      editor={editor}
      options={{
        placement: "top",
        offset: 8,
        // The menu is positioned outside the scroll area (it isn't a containing block), so it's
        // never clipped and draws over everything. The scroll area is still the limit it
        // flips against: below the selection when there's no room above inside it. TipTap
        // runs flip before offset, so the padding stands in for the 8px gap.
        flip: { boundary: scrollTarget, padding: 8, fallbackPlacements: ["bottom"] },
        shift: { boundary: scrollTarget, padding: 12 },
        // Out of sight once the selection scrolls out of the scroll area.
        hide: { boundary: scrollTarget },
        scrollTarget,
        onHide: () => setMode("format"),
        // autoFocus misses when the field mounted while the menu was hidden (opened from the
        // context menu, before the editor had focus back).
        onShow: () => linkInput.current?.focus(),
      }}
      shouldShow={({ editor: current, state, view, element, from, to }) =>
        current.isEditable &&
        from !== to &&
        !(state.selection instanceof NodeSelection) &&
        !current.isActive("codeBlock") &&
        (view.hasFocus() || element.contains(document.activeElement))
      }
      className="doc-selection-menu z-30 flex items-center gap-0.5 rounded-lg border bg-popover p-1 text-popover-foreground shadow-md"
    >
      {mode === "link" ? (
        <LinkField editor={editor} inputRef={linkInput} onDone={() => setMode("format")} />
      ) : (
        <>
          <MenuButton Icon={Bold} label="Bold" shortcut={shortcut("B")} active={active?.bold ?? false} onPress={() => run((c) => c.toggleBold())} />
          <MenuButton Icon={Italic} label="Italic" shortcut={shortcut("I")} active={active?.italic ?? false} onPress={() => run((c) => c.toggleItalic())} />
          <MenuButton Icon={Link2} label="Link" shortcut={shortcut("K")} active={active?.link ?? false} onPress={() => setMode("link")} />
          <Separator orientation="vertical" className="mx-0.5 h-4" />
          <MenuButton Icon={Heading1} label="Heading 1" active={active?.h1 ?? false} onPress={() => run((c) => c.toggleHeading({ level: 1 }))} />
          <MenuButton Icon={Heading2} label="Heading 2" active={active?.h2 ?? false} onPress={() => run((c) => c.toggleHeading({ level: 2 }))} />
          <MenuButton Icon={Heading3} label="Heading 3" active={active?.h3 ?? false} onPress={() => run((c) => c.toggleHeading({ level: 3 }))} />
          <Separator orientation="vertical" className="mx-0.5 h-4" />
          <MenuButton Icon={List} label="Bulleted list" active={active?.bullet ?? false} onPress={() => run((c) => c.toggleBulletList())} />
          <MenuButton Icon={ListOrdered} label="Numbered list" active={active?.ordered ?? false} onPress={() => run((c) => c.toggleOrderedList())} />
        </>
      )}
    </BubbleMenu>
  );
}
