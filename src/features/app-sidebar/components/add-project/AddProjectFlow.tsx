import { Folder, FolderGit2, LoaderCircle, Plus, Search } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { commands, type FolderEntry, type FolderListing, type FolderLocations } from "@/lib/bindings";
import {
  createRepository,
  linkRepository,
  pathKey,
  unarchiveProject,
  useProjects,
  type Outcome,
  type Project,
} from "@/features/projects";
import { useElementHeight } from "@/shared";
import { useDefaultProjectsDir } from "../../hooks/useDefaultProjectsDir";
import { DirectoryButton } from "./DirectoryButton";
import { FolderBrowser } from "@/components/folder-picker";
import { PressButton } from "./PressButton";
import { ProjectOption } from "./ProjectOption";
import { HEIGHT, view } from "./motion";

const DIR_KEY = "foundry:projects-folder";

function readDir(): string | null {
  try {
    return localStorage.getItem(DIR_KEY);
  } catch {
    return null;
  }
}

function saveDir(path: string) {
  try {
    localStorage.setItem(DIR_KEY, path);
  } catch {
    // Not remembered; the folder just has to be picked again next time.
  }
}

type Load = { status: "ready"; listing: FolderListing } | { status: "error"; message: string };

async function fetchListing(path: string): Promise<Load> {
  try {
    const result = await commands.listFolders(path);
    return result.status === "ok"
      ? { status: "ready", listing: result.data }
      : { status: "error", message: result.error.message };
  } catch (error) {
    return { status: "error", message: error instanceof Error ? error.message : String(error) };
  }
}

type Option =
  | { kind: "create"; key: string; name: string }
  | { kind: "folder"; key: string; folder: FolderEntry; project?: Project };

/**
 * The dialog's content: a search field over the folders in the projects folder.
 * Picking a folder links it; typing a new name offers to create it there.
 * Mounted only while the dialog is open, so every open starts fresh.
 */
export function AddProjectFlow({ onDone }: { onDone: (project: Project) => void }) {
  const projects = useProjects();
  const defaultDir = useDefaultProjectsDir();
  /** A folder picked in this dialog; until then the settings default, else the last pick. */
  const [picked, setPicked] = useState<string | null>(null);
  const settingsLoading = defaultDir === undefined;
  const dir = picked ?? (settingsLoading ? null : (defaultDir ?? readDir()));
  const [browsing, setBrowsing] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  /** Key of the option whose request is in flight; input and rows wait for it. */
  const [busy, setBusy] = useState<string | null>(null);
  const [settled, setSettled] = useState<{ dir: string; load: Load } | null>(null);
  const [locations, setLocations] = useState<FolderLocations | null>(null);

  const [measureRef, height] = useElementHeight<HTMLDivElement>();
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();

  useEffect(() => {
    let cancelled = false;
    commands
      .folderLocations()
      .then((result) => {
        if (!cancelled && result.status === "ok") setLocations(result.data);
      })
      .catch(() => {
        // Outside Tauri (plain browser): the picker shows its own error.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!dir) return;
    let cancelled = false;
    fetchListing(dir).then((load) => {
      if (!cancelled) setSettled({ dir, load });
    });
    return () => {
      cancelled = true;
    };
  }, [dir]);

  // On open (once the settings default is known) and back from the folder browser:
  // typing goes straight to search.
  useEffect(() => {
    if (!browsing) inputRef.current?.focus({ preventScroll: true });
  }, [browsing, settingsLoading]);

  const load = dir && settled?.dir === dir ? settled.load : null;
  const loading = settingsLoading || (dir !== null && load === null);

  const options = useMemo<Option[]>(() => {
    if (load?.status !== "ready") return [];
    const byPath = new Map(projects.map((project) => [pathKey(project.repoPath), project]));
    const name = query.trim();
    const needle = name.toLowerCase();

    const folders: Option[] = load.listing.folders
      .filter((folder) => folder.name.toLowerCase().includes(needle))
      // Git repositories first: they're the likeliest projects. Stable, so names stay sorted.
      .sort((a, b) => Number(b.isGitRepo) - Number(a.isGitRepo))
      .map((folder) => ({
        kind: "folder",
        key: folder.path,
        folder,
        project: byPath.get(pathKey(folder.path)),
      }));

    const exists = load.listing.folders.some((folder) => folder.name.toLowerCase() === needle);
    // Name rules are the backend's: it answers with invalid_input if it can't use the name.
    const canCreate = name !== "" && !exists;
    return canCreate ? [{ kind: "create", key: "create", name }, ...folders] : folders;
  }, [load, projects, query]);

  const active = Math.min(activeIndex, Math.max(options.length - 1, 0));
  const optionId = (index: number) => `${listId}-${index}`;

  // Keep the keyboard-highlighted row in view.
  useEffect(() => {
    document.getElementById(optionId(active))?.scrollIntoView({ block: "nearest" });
  });

  const choose = async (option: Option | undefined) => {
    if (!option || !dir || busy) return;
    setBusy(option.key);
    setError(null);

    let outcome: Outcome<Project>;
    if (option.kind === "create") {
      const created = await createRepository(option.name, dir);
      outcome = created.ok ? { ok: true, data: created.data.project } : created;
    } else if (option.project?.archived) {
      outcome = await unarchiveProject(option.project.id);
    } else if (option.project) {
      outcome = { ok: true, data: option.project };
    } else {
      const linked = await linkRepository(option.folder.path);
      outcome = linked.ok ? { ok: true, data: linked.data.project } : linked;
    }

    setBusy(null);
    // The backend's own message, verbatim (folder_exists, invalid_input, already_linked, …).
    if (outcome.ok) onDone(outcome.data);
    else setError(outcome.error.message);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    const move = (to: number) => {
      event.preventDefault();
      setActiveIndex(Math.max(0, Math.min(options.length - 1, to)));
    };
    if (event.key === "ArrowDown") move(active + 1);
    else if (event.key === "ArrowUp") move(active - 1);
    else if (event.key === "Enter") {
      event.preventDefault();
      void choose(options[active]);
    }
  };

  const pickDir = (path: string) => {
    saveDir(path);
    setPicked(path);
    setActiveIndex(0);
    setError(null);
    setBrowsing(false);
  };

  // Into the browser moves forward (content enters from the right), back out reverses.
  const direction = browsing ? 1 : -1;
  const trimmed = query.trim();

  return (
    <>
      <DialogTitle className="sr-only">Add project</DialogTitle>
      <DialogDescription className="sr-only">
        Search the folders in your projects folder, pick one to add it, or type a new name to create a repository.
      </DialogDescription>

      <motion.div initial={false} animate={{ height }} transition={HEIGHT} className="overflow-hidden">
        <div ref={measureRef} className="relative">
          <AnimatePresence initial={false} mode="popLayout" custom={direction}>
            {browsing ? (
              <motion.div
                key="browse"
                custom={direction}
                variants={view}
                initial="hidden"
                animate="shown"
                exit="gone"
                className="relative pt-4"
              >
                <p className="px-5 pb-3 text-[13px] font-medium">Where do your projects live?</p>
                <FolderBrowser
                  initialPath={dir}
                  locations={locations}
                  confirmVerb="Use"
                  onSelect={pickDir}
                  onCancel={() => setBrowsing(false)}
                />
              </motion.div>
            ) : (
              <motion.div
                key="search"
                custom={direction}
                variants={view}
                initial="hidden"
                animate="shown"
                exit="gone"
                className="relative"
              >
                <div className="flex items-center gap-2 border-b border-border/70 py-2.5 pr-2.5 pl-4">
                  <Search aria-hidden className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
                  <input
                    ref={inputRef}
                    value={query}
                    onChange={(event) => {
                      setQuery(event.target.value);
                      setActiveIndex(0);
                      setError(null);
                    }}
                    onKeyDown={onKeyDown}
                    disabled={!dir}
                    readOnly={busy !== null}
                    placeholder={dir ? "Search or create a project…" : "Choose a folder to search"}
                    autoComplete="off"
                    spellCheck={false}
                    role="combobox"
                    aria-label="Search projects"
                    aria-expanded={options.length > 0}
                    aria-controls={listId}
                    aria-activedescendant={options.length > 0 ? optionId(active) : undefined}
                    className="h-8 min-w-0 flex-1 bg-transparent text-[14px] outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed"
                  />
                  <DirectoryButton dir={dir} onClick={() => setBrowsing(true)} />
                </div>

                <div className="max-h-80 overflow-y-auto overscroll-contain p-1.5">
                  {loading ? (
                    <div className="flex items-center justify-center py-10">
                      <LoaderCircle aria-label="Loading" className="size-4 animate-spin text-muted-foreground" />
                    </div>
                  ) : !dir ? (
                    <div className="flex flex-col items-center gap-3 px-6 py-10 text-center">
                      <p className="text-[13px] text-muted-foreground">Choose the folder where your projects live.</p>
                      <PressButton size="sm" variant="outline" onClick={() => setBrowsing(true)}>
                        Choose folder
                      </PressButton>
                    </div>
                  ) : load?.status === "error" ? (
                    <div className="flex flex-col items-center gap-3 px-6 py-10 text-center">
                      <p className="text-[13px] text-muted-foreground">{load.message}</p>
                      <PressButton size="sm" variant="outline" onClick={() => setBrowsing(true)}>
                        Choose another folder
                      </PressButton>
                    </div>
                  ) : options.length === 0 ? (
                    <p className="px-3 py-10 text-center text-[13px] text-muted-foreground">
                      {trimmed ? "No folders match" : "This folder is empty. Type a name to create a project."}
                    </p>
                  ) : (
                    <ul id={listId} role="listbox" aria-label="Projects" className="flex flex-col">
                      {options.map((option, index) => (
                        <ProjectOption
                          key={option.key}
                          id={optionId(index)}
                          active={index === active}
                          busy={busy === option.key}
                          disabled={busy !== null}
                          onHover={() => setActiveIndex(index)}
                          onChoose={() => void choose(option)}
                          {...(option.kind === "create"
                            ? {
                                icon: Plus,
                                label: (
                                  <>
                                    Create <span className="font-medium">“{option.name}”</span>
                                  </>
                                ),
                                hint: "New repository",
                              }
                            : {
                                icon: option.folder.isGitRepo ? FolderGit2 : Folder,
                                label: option.folder.name,
                                hint: option.project?.archived
                                  ? "Archived · restore"
                                  : option.project
                                    ? "Added"
                                    : option.folder.isGitRepo
                                      ? "git"
                                      : undefined,
                              })}
                        />
                      ))}
                    </ul>
                  )}
                </div>

                {error && (
                  <p role="alert" className="px-5 pb-3 text-[12px] text-destructive">
                    {error}
                  </p>
                )}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </motion.div>
    </>
  );
}
