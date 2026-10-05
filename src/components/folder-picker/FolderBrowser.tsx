import { ArrowUp, HardDrive, House, LoaderCircle } from "lucide-react";
import { motion, useReducedMotionConfig } from "motion/react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { commands, type FolderListing, type FolderLocations } from "@/lib/bindings";
import { basename } from "./basename";
import { PressButton, usePressScale } from "@/shared";
import { FolderRow } from "./FolderRow";
import { LocationChip } from "./LocationChip";

export interface FolderBrowserProps {
  /** Folder to open first; null opens the home folder. */
  initialPath: string | null;
  locations: FolderLocations | null;
  /** Primary action, e.g. "Use" → "Use “projects”". */
  confirmVerb: string;
  onSelect: (path: string) => void;
  onCancel: () => void;
  /** Label of the secondary button that calls `onCancel`. */
  cancelLabel?: string;
}

type Load =
  | { status: "loading" }
  | { status: "ready"; listing: FolderListing }
  | { status: "error"; message: string };

async function fetchListing(path: string | null): Promise<Load> {
  try {
    const result = await commands.listFolders(path);
    return result.status === "ok"
      ? { status: "ready", listing: result.data }
      : { status: "error", message: result.error.message };
  } catch (error) {
    return { status: "error", message: error instanceof Error ? error.message : String(error) };
  }
}

/** In-app folder picker: styleable, unlike the native OS dialog. */
export function FolderBrowser({
  initialPath,
  locations,
  confirmVerb,
  onSelect,
  onCancel,
  cancelLabel = "Back",
}: FolderBrowserProps) {
  const reducedMotion = useReducedMotionConfig();
  const [path, setPath] = useState(initialPath);
  // The latest settled request; it's loading while that isn't for the current path.
  const [settled, setSettled] = useState<{ path: string | null; load: Load } | null>(null);
  // Keep showing the last good listing while the next one loads, so nothing flashes.
  const [shown, setShown] = useState<FolderListing | null>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const upRef = usePressScale<HTMLButtonElement>();

  const focusRow = (delta: 1 | -1 | "first" | "last") => {
    const rows = Array.from(listRef.current?.querySelectorAll<HTMLElement>("[data-folder-row]") ?? []);
    if (rows.length === 0) return;
    const current = rows.indexOf(document.activeElement as HTMLElement);
    const index =
      delta === "first" ? 0 : delta === "last" ? rows.length - 1 : Math.min(rows.length - 1, Math.max(0, current + delta));
    rows[index]?.focus();
  };

  useEffect(() => {
    let cancelled = false;
    fetchListing(path).then((load) => {
      if (cancelled) return;
      setSettled({ path, load });
      if (load.status === "ready") setShown(load.listing);
    });
    return () => {
      cancelled = true;
    };
  }, [path]);

  // Opening a folder unmounts the row that had focus; keep keyboard users in the list.
  useEffect(() => {
    const active = document.activeElement;
    if (!active || active === document.body || active.getAttribute("role") === "dialog") {
      focusRow("first");
    }
  }, [shown]);

  const listing = shown;
  const load: Load = settled?.path === path ? settled.load : { status: "loading" };
  const loading = load.status === "loading";
  const error = load.status === "error" ? load.message : null;

  const onListKeyDown = (event: KeyboardEvent) => {
    const keys: Record<string, () => void> = {
      ArrowDown: () => focusRow(1),
      ArrowUp: () => focusRow(-1),
      Home: () => focusRow("first"),
      End: () => focusRow("last"),
      Backspace: () => listing?.parent && setPath(listing.parent),
    };
    const action = keys[event.key];
    if (action) {
      event.preventDefault();
      action();
    }
  };

  const name = listing ? basename(listing.path) : "";

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-2 px-5 pb-2">
        <button
          ref={upRef}
          type="button"
          aria-label="Up one folder"
          disabled={!listing?.parent}
          onClick={() => listing?.parent && setPath(listing.parent)}
          className="flex size-8 shrink-0 items-center justify-center rounded-lg text-muted-foreground outline-none transition-colors hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/40 disabled:pointer-events-none disabled:opacity-40"
        >
          <ArrowUp className="size-4" strokeWidth={1.75} />
        </button>
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-[13px] font-medium">{name || " "}</span>
          <span dir="rtl" className="truncate text-left text-[11.5px] text-muted-foreground">
            <bdi dir="ltr">{listing?.path ?? " "}</bdi>
          </span>
        </div>
        <LoaderCircle
          aria-hidden
          className={`size-3.5 shrink-0 animate-spin text-muted-foreground transition-opacity duration-200 ${loading ? "opacity-100 delay-150" : "opacity-0"}`}
        />
      </div>

      {locations && (
        <div className="flex gap-1 overflow-x-auto px-5 pb-2 [scrollbar-width:none]">
          <LocationChip
            icon={House}
            label="Home"
            active={listing?.path === locations.home}
            onClick={() => setPath(locations.home)}
          />
          {locations.roots.map((root) => (
            <LocationChip
              key={root}
              icon={HardDrive}
              label={root.replace(/\\$/, "")}
              active={listing?.path === root}
              onClick={() => setPath(root)}
            />
          ))}
        </div>
      )}

      <div className="mx-3 h-72 overflow-y-auto overscroll-contain">
        {error ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
            <p className="text-[13px] text-muted-foreground">{error}</p>
            {locations && (
              <PressButton size="sm" variant="outline" onClick={() => setPath(locations.home)}>
                Go to home folder
              </PressButton>
            )}
          </div>
        ) : listing && listing.folders.length === 0 ? (
          <div className="flex h-full items-center justify-center text-[13px] text-muted-foreground">
            No folders here
          </div>
        ) : (
          <motion.ul
            // A new list per folder: fades in, never slides, so navigation feels instant.
            key={listing?.path ?? "loading"}
            ref={listRef}
            aria-label={name ? `Folders in ${name}` : "Folders"}
            onKeyDown={onListKeyDown}
            initial={reducedMotion ? false : { opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.14, ease: "easeOut" }}
            className="flex flex-col"
          >
            {listing?.folders.map((folder) => (
              <FolderRow key={folder.path} folder={folder} onOpen={setPath} />
            ))}
          </motion.ul>
        )}
      </div>

      <div className="flex items-center justify-between gap-2 px-6 pt-4 pb-6">
        <span className="min-w-0 truncate text-[12px] text-muted-foreground">
          {listing?.isGitRepo ? "This folder is a git repository" : ""}
        </span>
        <div className="flex shrink-0 gap-2">
          <PressButton variant="ghost" size="sm" onClick={onCancel}>
            {cancelLabel}
          </PressButton>
          <PressButton
            size="sm"
            disabled={!listing || loading || error !== null}
            onClick={() => listing && onSelect(listing.path)}
            className="max-w-56"
          >
            <span className="truncate">
              {confirmVerb} “{name}”
            </span>
          </PressButton>
        </div>
      </div>
    </div>
  );
}
