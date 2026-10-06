import { LoaderCircle, UserPlus } from "lucide-react";
import { useId, useState, type KeyboardEvent } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { CustomRole, HandbookRole, TeamWritten } from "@/lib/bindings";
import { PressButton, usePressScale } from "@/shared";
import type { HandbookRoles } from "../hooks/useHandbookRoles";
import type { Outcome } from "../hooks/call";
import type { TeamController } from "../hooks/useTeam";
import { cleanCustomRole, EMPTY_CUSTOM_ROLE, rosterAgent } from "../lib/draft";
import { CustomRoleFields } from "./CustomRoleFields";
import { ErrorLine } from "./ErrorLine";
import { HandbookRoleList } from "./HandbookRoleList";

type Source = "handbook" | "custom";

/**
 * "Add agent" on a confirmed team: a handbook role or a custom one, plus a display name, sent
 * through `addAgentManually`. The card appears once that succeeds.
 */
export function AddAgentButton({
  handbook,
  addAgent,
  onAdded,
}: {
  handbook: HandbookRoles;
  addAgent: TeamController["addAgent"];
  onAdded: (written: TeamWritten) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <PressButton
        size="sm"
        variant="outline"
        className="w-fit rounded-md"
        onClick={() => {
          handbook.reload();
          setOpen(true);
        }}
      >
        <UserPlus data-icon="inline-start" />
        Add agent
      </PressButton>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-lg">
          <AddAgentBody
            handbook={handbook}
            onClose={() => setOpen(false)}
            onSubmit={async (agent) => {
              const outcome = await addAgent(agent);
              if (outcome.ok) {
                setOpen(false);
                onAdded(outcome.data);
              }
              return outcome;
            }}
          />
        </DialogContent>
      </Dialog>
    </>
  );
}

function AddAgentBody({
  handbook,
  onClose,
  onSubmit,
}: {
  handbook: HandbookRoles;
  onClose: () => void;
  onSubmit: (agent: ReturnType<typeof rosterAgent>) => Promise<Outcome<TeamWritten>>;
}) {
  const id = useId();
  const [source, setSource] = useState<Source>("handbook");
  const [role, setRole] = useState<HandbookRole | null>(null);
  const [custom, setCustom] = useState<CustomRole>(EMPTY_CUSTOM_ROLE);
  /** Null while the name follows the role's title; a string once typed. */
  const [typedName, setTypedName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const cleanedCustom = cleanCustomRole(custom);
  const defaultName = source === "handbook" ? (role?.title ?? "") : custom.title;
  const name = typedName ?? defaultName;
  const ready = !busy && name.trim() !== "" && (source === "handbook" ? role !== null : cleanedCustom !== null);

  const submit = async () => {
    if (!ready) return;
    setBusy(true);
    setError(null);
    const outcome = await onSubmit(
      rosterAgent(
        source === "handbook"
          ? { handbookRoleId: role!.id, customRole: null, displayName: name, title: role!.title, relatedSliceIds: [] }
          : { handbookRoleId: null, customRole: cleanedCustom, displayName: name, title: cleanedCustom!.title, relatedSliceIds: [] },
      ),
    );
    setBusy(false);
    if (!outcome.ok) setError(outcome.error.message);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void submit();
    }
  };

  return (
    <div className="contents" onKeyDown={onKeyDown}>
      <DialogHeader>
        <DialogTitle>Add agent</DialogTitle>
        <DialogDescription className="text-[13px] leading-snug">
          The new agent joins the confirmed team. Everyone already on it stays as they are.
        </DialogDescription>
      </DialogHeader>

      <div className="flex min-h-0 flex-col gap-4">
        <ToggleGroup
          value={[source]}
          onValueChange={(next) => {
            if (next[0]) setSource(next[0] as Source);
          }}
          spacing={0}
          aria-label="Where the role comes from"
          className="gap-0.5 rounded-lg bg-muted p-0.5"
        >
          <SourceItem value="handbook" label="From handbook" />
          <SourceItem value="custom" label="Custom role" />
        </ToggleGroup>

        {source === "handbook" ? (
          <HandbookRoleList handbook={handbook} selectedId={role?.id ?? null} onPick={setRole} autoFocus />
        ) : (
          <CustomRoleFields value={custom} onChange={setCustom} autoFocus />
        )}

        <div className="flex flex-col gap-1.5">
          <Label htmlFor={`${id}-name`} className="text-[12.5px]">
            Display name
          </Label>
          <Input
            id={`${id}-name`}
            value={name}
            onChange={(event) => setTypedName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void submit();
              }
            }}
            placeholder={source === "handbook" ? "Pick a role, or type a name" : "Defaults to the title"}
            className="text-[13px]"
          />
        </div>

        {error && <ErrorLine message={error} />}
      </div>

      <DialogFooter>
        <PressButton type="button" variant="ghost" onClick={onClose}>
          Cancel
        </PressButton>
        <PressButton type="button" disabled={!ready} onClick={() => void submit()}>
          {busy && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
          Add agent
        </PressButton>
      </DialogFooter>
    </div>
  );
}

function SourceItem({ value, label }: { value: Source; label: string }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.96 });
  return (
    <ToggleGroupItem
      ref={ref}
      value={value}
      className="h-7 rounded-md! px-3 text-[12.5px] text-muted-foreground hover:bg-transparent hover:text-foreground aria-pressed:bg-background aria-pressed:text-foreground aria-pressed:shadow-sm"
    >
      {label}
    </ToggleGroupItem>
  );
}
