import { BookUser, Check, LoaderCircle, Plus, TriangleAlert, X } from "lucide-react";
import { useReducedMotionConfig } from "motion/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { HandbookRole, RolesImported } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { DropOverlay, PressButton, useFileDrop } from "@/shared";
import { useRoleHandbook } from "../hooks/useRoleHandbook";
import { ReplaceRolesDialog, type RoleConflict } from "./ReplaceRolesDialog";
import { RoleFormatPopover } from "./RoleFormatPopover";
import { RoleSheet, type RoleSheetTarget } from "./RoleSheet";

/** How long the rows a drop added or replaced stay highlighted. */
const HIGHLIGHT_MS = 2200;
/** How long the "Added …" notice stays. */
const NOTICE_MS = 6000;

/**
 * The Role Handbook: the roles the Product Lead picks from. App-wide, not per project. The
 * backend creates the starter roles on its first read; the UI never adds any by itself.
 * Drop `.md` files anywhere on it to add one role per file.
 */
export function RoleHandbookView() {
  const data = useRoleHandbook();
  const { roles, loadError, reload, addFromPaths } = data;
  const rootRef = useRef<HTMLDivElement>(null);
  const [target, setTarget] = useState<RoleSheetTarget>(null);
  const [adding, setAdding] = useState(false);
  const [dropError, setDropError] = useState<string | null>(null);
  const [conflict, setConflict] = useState<RoleConflict | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [highlighted, setHighlighted] = useState<ReadonlySet<string>>(new Set());
  const reduceMotion = useReducedMotionConfig();
  const scrollPending = useRef(false);

  const landed = ({ added, replaced }: RolesImported) => {
    setNotice(importedNotice(added.length, replaced.length));
    setHighlighted(new Set([...added, ...replaced]));
    scrollPending.current = true;
  };

  const dragging = useFileDrop(rootRef, async (paths) => {
    setDropError(null);
    setNotice(null);
    setAdding(true);
    const outcome = await addFromPaths(paths, false);
    setAdding(false);
    if (outcome.ok) landed(outcome.data);
    else if (outcome.error.code === "role_exists") setConflict({ paths, message: outcome.error.message });
    // All or nothing: the backend's own message (e.g. a file that isn't .md, bad frontmatter).
    else setDropError(outcome.error.message);
  });

  // Scroll to the first highlighted row in list order, once per drop (the list has usually
  // been re-read already; if not, this runs again when it arrives).
  useEffect(() => {
    if (!scrollPending.current) return;
    const first = roles?.find((role) => highlighted.has(role.id));
    if (!first) return;
    scrollPending.current = false;
    rootRef.current
      ?.querySelector(`[data-role-id="${CSS.escape(first.id)}"]`)
      ?.scrollIntoView({ block: "nearest", behavior: reduceMotion ? "auto" : "smooth" });
  }, [highlighted, roles, reduceMotion]);

  useEffect(() => {
    if (highlighted.size === 0) return;
    const timer = setTimeout(() => setHighlighted(new Set()), HIGHLIGHT_MS);
    return () => clearTimeout(timer);
  }, [highlighted]);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), NOTICE_MS);
    return () => clearTimeout(timer);
  }, [notice]);

  const newRole = (
    <PressButton size="sm" variant="outline" className="h-8 rounded-md" onClick={() => setTarget({ mode: "new" })}>
      <Plus data-icon="inline-start" />
      New role
    </PressButton>
  );

  return (
    <div ref={rootRef} className="relative flex min-h-[26rem] flex-col gap-3 pt-3">
      <div className="flex min-h-8 items-center justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <p className="text-[12.5px] text-muted-foreground">
            {roles && roles.length > 0 ? `The roles the Product Lead picks from · ${roles.length}` : "The roles the Product Lead picks from"}
          </p>
          <DropHint adding={adding} />
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <RoleFormatPopover />
          {roles && roles.length > 0 && newRole}
        </div>
      </div>

      {loadError && (
        <ErrorLine message={loadError}>
          <PressButton size="xs" variant="outline" className="rounded-md" onClick={() => void reload()}>
            Try again
          </PressButton>
        </ErrorLine>
      )}
      {dropError && (
        <ErrorLine message={dropError}>
          <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={() => setDropError(null)} className="rounded-md">
            <X aria-hidden />
          </PressButton>
        </ErrorLine>
      )}
      {notice && (
        <div role="status" className="flex items-center gap-2 rounded-lg bg-muted py-1.5 pr-1.5 pl-3 text-[12.5px]">
          <Check aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
          <p className="min-w-0 flex-1">{notice}</p>
          <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={() => setNotice(null)} className="rounded-md">
            <X aria-hidden />
          </PressButton>
        </div>
      )}

      {!roles && !loadError ? (
        <div className="flex justify-center py-10">
          <LoaderCircle aria-label="Loading the role handbook" className="size-4 animate-spin text-muted-foreground" />
        </div>
      ) : roles && roles.length === 0 ? (
        <div className="flex min-h-56 flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-muted-foreground/35 px-6 text-center">
          <BookUser aria-hidden className="size-5 text-muted-foreground" />
          <div className="flex flex-col gap-1">
            <p className="max-w-72 text-[12.5px] leading-snug text-muted-foreground">
              No roles left. Add one so the Product Lead has someone to pick.
            </p>
            <p className="text-[12px] text-muted-foreground/80">Drop .md files to add roles</p>
          </div>
          {newRole}
        </div>
      ) : (
        roles && (
          <ul className="flex flex-col" aria-label="Roles">
            {roles.map((role) => (
              <RoleRow
                key={role.id}
                role={role}
                highlighted={highlighted.has(role.id)}
                onOpen={() => setTarget({ mode: "edit", id: role.id })}
              />
            ))}
          </ul>
        )
      )}

      {roles && <DropOverlay active={dragging} />}

      {roles && <RoleSheet roles={roles} target={target} onTargetChange={setTarget} data={data} />}
      <ReplaceRolesDialog
        conflict={conflict}
        onClose={() => setConflict(null)}
        onReplace={async (paths) => {
          const outcome = await addFromPaths(paths, true);
          if (outcome.ok) landed(outcome.data);
          // On an error the dialog stays open and shows the message as-is.
          return outcome;
        }}
      />
    </div>
  );
}

function DropHint({ adding }: { adding: boolean }) {
  return (
    <p className="flex items-center gap-1.5 text-[12px] text-muted-foreground/80">
      {adding && <LoaderCircle aria-hidden className="size-3 animate-spin" />}
      {adding ? "Adding roles…" : "Drop .md files to add roles"}
    </p>
  );
}

function importedNotice(added: number, replaced: number): string {
  const roles = (count: number) => `${count} ${count === 1 ? "role" : "roles"}`;
  if (added > 0 && replaced > 0) return `Added ${roles(added)} and replaced ${roles(replaced)}.`;
  if (replaced > 0) return `Replaced ${roles(replaced)}.`;
  return `Added ${roles(added)}.`;
}

function RoleRow({ role, highlighted, onOpen }: { role: HandbookRole; highlighted: boolean; onOpen: () => void }) {
  return (
    <li data-role-id={role.id} className="relative border-b border-border/60 last:border-b-0">
      {/* Just added or replaced by a drop: a tint that fades (opacity only, nothing reflows). */}
      <span
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-x-0 inset-y-1 rounded-md bg-primary/8 opacity-0 ring-1 ring-primary/25 transition-opacity duration-700 motion-reduce:transition-none",
          highlighted && "opacity-100 duration-150",
        )}
      />
      <PressButton
        variant="ghost"
        pressScale={0.99}
        onClick={onOpen}
        className="relative my-1 h-auto w-full flex-col items-stretch gap-0.5 rounded-md px-2.5 py-2.5 text-left font-normal whitespace-normal active:not-aria-[haspopup]:translate-y-0"
      >
        <span className="truncate text-[13px] font-medium">{role.title || "Untitled role"}</span>
        {role.description && <span className="line-clamp-2 text-[12.5px] text-muted-foreground">{role.description}</span>}
      </PressButton>
    </li>
  );
}

function ErrorLine({ message, children }: { message: string; children?: ReactNode }) {
  return (
    <div role="alert" className="flex items-start gap-2 rounded-lg bg-destructive/8 py-2 pr-1.5 pl-3 text-[12.5px]">
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 leading-snug">{message}</p>
      {children}
    </div>
  );
}
