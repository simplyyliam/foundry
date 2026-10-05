import { BookUser, LoaderCircle, Plus, TriangleAlert } from "lucide-react";
import { useState } from "react";
import type { HandbookRole } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useRoleHandbook } from "../hooks/useRoleHandbook";
import { RoleSheet, type RoleSheetTarget } from "./RoleSheet";

/**
 * The Role Handbook: the roles the Product Lead picks from. App-wide, not per project. The
 * backend creates the starter roles on its first read; the UI never adds any by itself.
 */
export function RoleHandbookView() {
  const data = useRoleHandbook();
  const { roles, loadError, reload } = data;
  const [target, setTarget] = useState<RoleSheetTarget>(null);

  const newRole = (
    <PressButton size="sm" variant="outline" className="h-8 rounded-md" onClick={() => setTarget({ mode: "new" })}>
      <Plus data-icon="inline-start" />
      New role
    </PressButton>
  );

  return (
    <div className="flex flex-col gap-3 pt-3">
      <div className="flex min-h-8 items-center justify-between gap-4">
        <p className="text-[12.5px] text-muted-foreground">
          {roles && roles.length > 0 ? `The roles the Product Lead picks from · ${roles.length}` : "The roles the Product Lead picks from"}
        </p>
        {roles && roles.length > 0 && newRole}
      </div>

      {loadError && (
        <div role="alert" className="flex items-start gap-2 rounded-lg bg-destructive/8 py-2 pr-1.5 pl-3 text-[12.5px]">
          <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
          <p className="min-w-0 flex-1 leading-snug">{loadError}</p>
          <PressButton size="xs" variant="outline" className="rounded-md" onClick={() => void reload()}>
            Try again
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
          <p className="max-w-72 text-[12.5px] leading-snug text-muted-foreground">
            No roles left. Add one so the Product Lead has someone to pick.
          </p>
          {newRole}
        </div>
      ) : (
        roles && (
          <ul className="flex flex-col" aria-label="Roles">
            {roles.map((role) => (
              <RoleRow key={role.id} role={role} onOpen={() => setTarget({ mode: "edit", id: role.id })} />
            ))}
          </ul>
        )
      )}

      {roles && <RoleSheet roles={roles} target={target} onTargetChange={setTarget} data={data} />}
    </div>
  );
}

function RoleRow({ role, onOpen }: { role: HandbookRole; onOpen: () => void }) {
  return (
    <li className="border-b border-border/60 last:border-b-0">
      <PressButton
        variant="ghost"
        pressScale={0.99}
        onClick={onOpen}
        className="my-1 h-auto w-full flex-col items-stretch gap-0.5 rounded-md px-2.5 py-2.5 text-left font-normal whitespace-normal active:not-aria-[haspopup]:translate-y-0"
      >
        <span className="truncate text-[13px] font-medium">{role.title || "Untitled role"}</span>
        {role.description && <span className="line-clamp-2 text-[12.5px] text-muted-foreground">{role.description}</span>}
      </PressButton>
    </li>
  );
}
