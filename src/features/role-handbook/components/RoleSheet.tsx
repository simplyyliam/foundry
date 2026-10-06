import { Trash2, TriangleAlert } from "lucide-react";
import { useId, useState, type ReactNode } from "react";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { HandbookRole, NewHandbookRole } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { RoleHandbookData } from "../hooks/useRoleHandbook";
import { PatternList } from "./PatternList";
import { RoleField } from "./RoleField";
import { useSaveState } from "../hooks/useSaveState";
import { SaveStatus } from "./SaveStatus";

/** Over this many words, the skill gets a warning: every agent in the role reads it each session. */
const LONG_SKILL_WORDS = 1500;

export type RoleSheetTarget = { mode: "new" } | { mode: "edit"; id: string } | null;

const ALWAYS_PROTECTED =
  "AGENTS.md, agents/**, docs/design/**, .foundry/plan.json and .claude/** are always protected for every agent, whatever you enter here.";

interface RoleSheetProps {
  roles: HandbookRole[];
  target: RoleSheetTarget;
  onTargetChange: (target: RoleSheetTarget) => void;
  data: Pick<RoleHandbookData, "add" | "update" | "remove">;
}

/** One role, editable (saves per field), or a blank one to add with "New role". */
export function RoleSheet({ roles, target, onTargetChange, data }: RoleSheetProps) {
  // Keep showing the last content while the sheet animates out.
  const [shown, setShown] = useState<RoleSheetTarget>(target);
  if (target !== null && target !== shown) setShown(target);
  const role = shown?.mode === "edit" ? (roles.find((candidate) => candidate.id === shown.id) ?? null) : null;
  const open = target !== null && (target.mode === "new" || role !== null);

  return (
    <Sheet open={open} onOpenChange={(next) => !next && onTargetChange(null)}>
      <SheetContent className="w-full gap-0 sm:max-w-lg">
        {shown?.mode === "new" ? (
          <NewRole
            key="new"
            onAdd={async (draft) => {
              const before = new Set(roles.map((existing) => existing.id));
              const outcome = await data.add(draft);
              if (outcome.ok) {
                // Carry on editing the role just added (saved per field from here on).
                const added = outcome.data.find((candidate) => !before.has(candidate.id));
                onTargetChange(added ? { mode: "edit", id: added.id } : null);
              }
              return outcome;
            }}
            onCancel={() => onTargetChange(null)}
          />
        ) : (
          role && <EditRole key={role.id} role={role} data={data} onRemoved={() => onTargetChange(null)} />
        )}
      </SheetContent>
    </Sheet>
  );
}

function EditRole({ role, data, onRemoved }: { role: HandbookRole; data: RoleSheetProps["data"]; onRemoved: () => void }) {
  const { state, track } = useSaveState();
  const save = (patch: Parameters<RoleHandbookData["update"]>[1]) => track(data.update(role.id, patch));

  return (
    <RoleForm
      title={role.title || "Untitled role"}
      status={<SaveStatus state={state} />}
      hint="Changes save on their own: text when you leave a field, patterns when you leave the list or remove one."
      footer={<RemoveRole role={role} remove={data.remove} onRemoved={onRemoved} />}
      fields={{
        title: { value: role.title, onCommit: (title) => save({ title }) },
        description: { value: role.description, onCommit: (description) => save({ description }) },
        skill: { value: role.skillTemplate, onCommit: (skillTemplate) => save({ skillTemplate }) },
        write: { patterns: role.typicalWriteGlobs, onCommit: (typicalWriteGlobs) => save({ typicalWriteGlobs }) },
        protected: { patterns: role.typicalProtectedGlobs, onCommit: (typicalProtectedGlobs) => save({ typicalProtectedGlobs }) },
      }}
    />
  );
}

const BLANK: NewHandbookRole = { title: "", description: "", skillTemplate: "", typicalWriteGlobs: [], typicalProtectedGlobs: [] };

function NewRole({
  onAdd,
  onCancel,
}: {
  onAdd: (role: NewHandbookRole) => Promise<{ ok: boolean; error?: { message: string } }>;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState<NewHandbookRole>(BLANK);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (change: Partial<NewHandbookRole>) => setDraft((current) => ({ ...current, ...change }));

  const submit = async () => {
    setBusy(true);
    setError(null);
    const clean = (rows: string[]) => rows.map((row) => row.trim()).filter(Boolean);
    const outcome = await onAdd({
      title: draft.title.trim(),
      description: draft.description.trim(),
      skillTemplate: draft.skillTemplate.trim(),
      typicalWriteGlobs: clean(draft.typicalWriteGlobs),
      typicalProtectedGlobs: clean(draft.typicalProtectedGlobs),
    });
    setBusy(false);
    // The backend's own rules and message (e.g. a missing title).
    if (!outcome.ok) setError(outcome.error?.message ?? null);
  };

  return (
    <RoleForm
      title="New role"
      hint="Nothing is saved until you add the role."
      footer={
        <div className="flex flex-col gap-2 border-t border-border/60 pt-4">
          {error && (
            <p role="alert" className="rounded-lg bg-destructive/8 px-3 py-2 text-[13px]">
              {error}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <PressButton variant="ghost" size="sm" onClick={onCancel}>
              Cancel
            </PressButton>
            <PressButton size="sm" disabled={busy} onClick={() => void submit()}>
              Add role
            </PressButton>
          </div>
        </div>
      }
      fields={{
        title: { value: draft.title, onChange: (title) => set({ title }) },
        description: { value: draft.description, onChange: (description) => set({ description }) },
        skill: { value: draft.skillTemplate, onChange: (skillTemplate) => set({ skillTemplate }) },
        write: { patterns: draft.typicalWriteGlobs, onChange: (typicalWriteGlobs) => set({ typicalWriteGlobs }) },
        protected: { patterns: draft.typicalProtectedGlobs, onChange: (typicalProtectedGlobs) => set({ typicalProtectedGlobs }) },
      }}
    />
  );
}

type TextBinding = { value: string } & Pick<Parameters<typeof RoleField>[0], "onCommit" | "onChange">;
type ListBinding = { patterns: string[] } & Pick<Parameters<typeof PatternList>[0], "onCommit" | "onChange">;

interface RoleFormProps {
  title: string;
  status?: ReactNode;
  hint: string;
  footer: ReactNode;
  fields: { title: TextBinding; description: TextBinding; skill: TextBinding; write: ListBinding; protected: ListBinding };
}

function RoleForm({ title, status, hint, footer, fields }: RoleFormProps) {
  const id = useId();

  return (
    <>
      <SheetHeader className="gap-1 pr-14">
        <SheetTitle className="truncate text-base">{title}</SheetTitle>
        <SheetDescription className="flex min-h-4.5 items-center gap-2 text-[12px]">{status}</SheetDescription>
      </SheetHeader>

      <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-6 pb-6"
      >
        <p className="rounded-lg bg-muted px-3 py-2 text-[12px] leading-snug text-muted-foreground">{hint}</p>

        <Field label="Title" htmlFor={`${id}-title`}>
          <RoleField id={`${id}-title`} placeholder="e.g. Frontend Engineer" {...fields.title} />
        </Field>
        <Field label="Description" htmlFor={`${id}-description`}>
          <RoleField id={`${id}-description`} multiline placeholder="One or two lines the Product Lead reads when picking roles" className="min-h-16" {...fields.description} />
        </Field>
        <Field label="Skill" htmlFor={`${id}-skill`} note="Who this role is and what it provides. Merged into the role file of every agent made from it.">
          <RoleField
            id={`${id}-skill`}
            multiline
            describedBy={`${id}-skill-note`}
            className="min-h-56 font-mono text-[12.5px] md:text-[12.5px]"
            below={(text) => wordCount(text) > LONG_SKILL_WORDS && <LongSkillWarning />}
            {...fields.skill}
          />
        </Field>
        <Field label="Write patterns" note="Files this role typically writes.">
          <PatternList label="Write patterns" placeholder="src/components/**" {...fields.write} />
        </Field>
        <Field label="Protected patterns" note="Files this role must not touch." after={ALWAYS_PROTECTED}>
          <PatternList label="Protected patterns" placeholder="src-tauri/**" {...fields.protected} />
        </Field>

        {footer}
      </div>
    </>
  );
}

function wordCount(text: string): number {
  const trimmed = text.trim();
  return trimmed === "" ? 0 : trimmed.split(/\s+/).length;
}

function LongSkillWarning() {
  return (
    <p className="flex items-start gap-1.5 text-[12px] leading-snug text-muted-foreground">
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0" />
      Every agent in this role reads this at the start of each session. Long skills use more of your Claude usage.
    </p>
  );
}

function Field({
  label,
  htmlFor,
  note,
  after,
  children,
}: {
  label: string;
  htmlFor?: string;
  note?: string;
  after?: string;
  children: ReactNode;
}) {
  const noteId = htmlFor ? `${htmlFor}-note` : undefined;
  return (
    <div className="flex flex-col gap-1.5">
      {htmlFor ? (
        <label htmlFor={htmlFor} className="text-[12px] font-medium text-muted-foreground">
          {label}
        </label>
      ) : (
        <span className="text-[12px] font-medium text-muted-foreground">{label}</span>
      )}
      {note && (
        <p id={noteId} className="-mt-1 text-[12px] text-muted-foreground/80">
          {note}
        </p>
      )}
      {children}
      {after && <p className="text-[12px] leading-snug text-muted-foreground">{after}</p>}
    </div>
  );
}

function RemoveRole({ role, remove, onRemoved }: { role: HandbookRole; remove: RoleHandbookData["remove"]; onRemoved: () => void }) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const confirm = async () => {
    setBusy(true);
    const outcome = await remove(role.id);
    setBusy(false);
    if (outcome.ok) onRemoved();
    else {
      setError(outcome.error.message);
      setConfirming(false);
    }
  };

  return (
    <section className="mt-1 flex flex-col gap-2 border-t border-border/60 pt-4">
      {confirming ? (
        <div className="flex flex-wrap items-center gap-2">
          <p className="mr-auto text-[13px]">Remove “{role.title || "Untitled role"}” from the handbook?</p>
          <PressButton variant="ghost" size="sm" onClick={() => setConfirming(false)}>
            Cancel
          </PressButton>
          <PressButton variant="destructive" size="sm" disabled={busy} onClick={() => void confirm()}>
            Remove
          </PressButton>
        </div>
      ) : (
        <PressButton
          variant="ghost"
          size="sm"
          className="w-fit text-destructive hover:text-destructive"
          onClick={() => {
            setError(null);
            setConfirming(true);
          }}
        >
          <Trash2 aria-hidden data-icon="inline-start" />
          Remove role
        </PressButton>
      )}
      {error && (
        <p role="alert" className="rounded-lg bg-destructive/8 px-3 py-2 text-[13px]">
          {error}
        </p>
      )}
    </section>
  );
}
