import { Check, Pencil, Trash2 } from "lucide-react";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Input } from "@/components/ui/input";
import type { HandbookRole, SliceView } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import { DEFAULT_AVATAR } from "../lib/avatar";
import type { DraftAgent } from "../lib/draft";
import { RoleChip } from "./RoleChip";

interface ProposalCardProps {
  draft: DraftAgent;
  roles: readonly HandbookRole[] | null;
  slices: readonly SliceView[];
  /** Another card uses the same display name. */
  duplicate: boolean;
  /** Opens in rename mode with the name selected (a card just added by hand). */
  startEditing: boolean;
  onChange: (patch: Pick<DraftAgent, "displayName" | "title">) => void;
  onRemove: () => void;
}

/**
 * One proposed agent: the default avatar, name, title, role chip, the Product Lead's
 * rationale and the slices it relates to. Rename edits the card live; Enter or Done ends it,
 * Escape puts back what was there when renaming started. Remove asks inline.
 */
export function ProposalCard({ draft, roles, slices, duplicate, startEditing, onChange, onRemove }: ProposalCardProps) {
  const id = useId();
  const [editing, setEditing] = useState<Pick<DraftAgent, "displayName" | "title"> | null>(() =>
    startEditing ? { displayName: draft.displayName, title: draft.title } : null,
  );
  const [confirmingRemove, setConfirmingRemove] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (startEditing) nameRef.current?.select();
  }, [startEditing]);

  const beginRename = () => {
    setEditing({ displayName: draft.displayName, title: draft.title });
    requestAnimationFrame(() => nameRef.current?.select());
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      setEditing(null);
    } else if (event.key === "Escape" && editing) {
      event.preventDefault();
      onChange(editing);
      setEditing(null);
    }
  };

  const emptyName = !draft.displayName.trim();
  const sliceName = (sliceId: string) => slices.find((slice) => slice.id === sliceId)?.name ?? sliceId;

  return (
    <li className="flex gap-3 rounded-xl bg-card p-4 ring-1 ring-foreground/8">
      <Avatar size="lg">
        <AvatarImage src={DEFAULT_AVATAR} alt="" />
        <AvatarFallback />
      </Avatar>

      <div className="flex min-w-0 flex-1 flex-col gap-2">
        {editing ? (
          <div className="flex flex-col gap-2 sm:flex-row">
            <Input
              ref={nameRef}
              aria-label="Display name"
              aria-invalid={emptyName || duplicate || undefined}
              aria-describedby={emptyName || duplicate ? `${id}-name-note` : undefined}
              value={draft.displayName}
              placeholder="Display name"
              onChange={(event) => onChange({ displayName: event.target.value, title: draft.title })}
              onKeyDown={onKeyDown}
              className="h-8 text-[13px] font-medium sm:max-w-52"
            />
            <Input
              aria-label="Title"
              value={draft.title}
              placeholder="Title"
              onChange={(event) => onChange({ displayName: draft.displayName, title: event.target.value })}
              onKeyDown={onKeyDown}
              className="h-8 flex-1 text-[13px]"
            />
            <PressButton size="sm" variant="ghost" className="h-8 rounded-md" onClick={() => setEditing(null)}>
              <Check data-icon="inline-start" />
              Done
            </PressButton>
          </div>
        ) : (
          <div className="flex min-w-0 items-start gap-2">
            <div className="flex min-w-0 flex-1 flex-col">
              <p className={cn("truncate text-[13.5px] font-medium", emptyName && "text-muted-foreground italic")}>
                {draft.displayName.trim() || "No name"}
              </p>
              <p className="truncate text-[12.5px] text-muted-foreground">{draft.title.trim() || "No title"}</p>
            </div>
            {!confirmingRemove && (
              <div className="-mt-1 -mr-1 flex shrink-0 gap-0.5">
                <PressButton
                  pressScale={0.9}
                  size="icon-sm"
                  variant="ghost"
                  aria-label={`Rename ${draft.displayName.trim() || "agent"}`}
                  onClick={beginRename}
                >
                  <Pencil className="size-3.5" />
                </PressButton>
                <PressButton
                  pressScale={0.9}
                  size="icon-sm"
                  variant="ghost"
                  aria-label={`Remove ${draft.displayName.trim() || "agent"}`}
                  onClick={() => setConfirmingRemove(true)}
                >
                  <Trash2 className="size-3.5" />
                </PressButton>
              </div>
            )}
          </div>
        )}

        {(emptyName || duplicate) && (
          <p id={`${id}-name-note`} className="text-[12px] text-destructive">
            {emptyName ? "Needs a display name." : "Another agent has this name."}
          </p>
        )}

        {confirmingRemove && (
          <div role="group" aria-label="Remove this agent?" className="flex flex-wrap items-center gap-2 rounded-lg bg-destructive/8 py-1.5 pr-1.5 pl-3">
            <p className="flex-1 text-[12.5px]">Remove {draft.displayName.trim() || "this agent"} from the proposal?</p>
            <PressButton size="xs" variant="ghost" className="rounded-md" autoFocus onClick={() => setConfirmingRemove(false)}>
              Keep
            </PressButton>
            <PressButton size="xs" variant="destructive" className="rounded-md" onClick={onRemove}>
              Remove
            </PressButton>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-1.5">
          <RoleChip handbookRoleId={draft.handbookRoleId} roles={roles} />
        </div>

        {draft.rationale && <p className="text-[12.5px] leading-snug text-pretty text-muted-foreground">{draft.rationale}</p>}

        {draft.relatedSliceIds.length > 0 && (
          <p className="text-[12px] leading-snug text-muted-foreground">
            <span className="font-medium text-foreground/80">Slices:</span> {draft.relatedSliceIds.map(sliceName).join(", ")}
          </p>
        )}
      </div>
    </li>
  );
}
