import { LoaderCircle, UserPlus } from "lucide-react";
import { useEffect, useId, useState, type KeyboardEvent } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { CustomRole, HandbookRole, TeamWritten } from "@/lib/bindings";
import { PressButton, useAddAgentDialog, usePressScale, type AddAgentRequest } from "@/shared";
import { useAddAgent, type AddAgentOutcome } from "../hooks/useAddAgent";
import { useHandbookRoles, type HandbookRoles } from "../hooks/useHandbookRoles";
import { cleanCustomRole, EMPTY_CUSTOM_ROLE, rosterAgent } from "../lib/draft";
import { CustomRoleFields } from "./CustomRoleFields";
import { ErrorLine } from "./ErrorLine";
import { HandbookRoleList } from "./HandbookRoleList";

type Source = "handbook" | "custom";

/** Setup's "Add agent" on a confirmed team: opens the app-wide Add agent dialog. */
export function AddAgentButton({ projectId, onAdded }: { projectId: string; onAdded: (written: TeamWritten) => void }) {
  const { openAddAgent } = useAddAgentDialog();
  return (
    <PressButton size="sm" variant="outline" className="w-fit rounded-md" onClick={() => openAddAgent(projectId, { onAdded })}>
      <UserPlus data-icon="inline-start" />
      Add agent
    </PressButton>
  );
}

/**
 * The one "Add agent" dialog, mounted in the app layout and opened through
 * `useAddAgentDialog` (Setup's button, Agent Chats' "+"): a handbook role or a custom one,
 * plus a display name, sent through `addAgentManually`. Every team view refreshes itself.
 */
export function AddAgentDialog() {
  const { request, close } = useAddAgentDialog();
  const handbook = useHandbookRoles();
  const addAgent = useAddAgent();
  // Keeps the body's project while the close animation runs.
  const [shown, setShown] = useState<AddAgentRequest | null>(request);
  if (request && request !== shown) setShown(request);

  const { reload } = handbook;
  useEffect(() => {
    if (request) reload();
  }, [request, reload]);

  return (
    <Dialog open={request !== null} onOpenChange={(open) => !open && close()}>
      <DialogContent className="sm:max-w-lg">
        {shown && (
          <AddAgentBody
            handbook={handbook}
            onClose={close}
            onSubmit={async (agent, saveRole) => {
              const outcome = await addAgent(shown.projectId, agent, saveRole);
              if (outcome.ok) {
                shown.onAdded?.(outcome.written);
                if (!outcome.handbookError) close();
              }
              return outcome;
            }}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function AddAgentBody({
  handbook,
  onClose,
  onSubmit,
}: {
  handbook: HandbookRoles;
  onClose: () => void;
  onSubmit: (agent: ReturnType<typeof rosterAgent>, saveRole: CustomRole | null) => Promise<AddAgentOutcome>;
}) {
  const id = useId();
  const [source, setSource] = useState<Source>("handbook");
  const [role, setRole] = useState<HandbookRole | null>(null);
  const [custom, setCustom] = useState<CustomRole>(EMPTY_CUSTOM_ROLE);
  /** Null while the name follows the role's title; a string once typed. */
  const [typedName, setTypedName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Custom roles only: also add the role to the Role Handbook once the agent exists. */
  const [saveToHandbook, setSaveToHandbook] = useState(false);
  /** Set when the agent was added but saving its role to the handbook failed. */
  const [handbookError, setHandbookError] = useState<string | null>(null);

  const cleanedCustom = cleanCustomRole(custom);
  const defaultName = source === "handbook" ? (role?.title ?? "") : custom.title;
  const name = typedName ?? defaultName;
  const ready = !busy && handbookError === null && name.trim() !== "" && (source === "handbook" ? role !== null : cleanedCustom !== null);

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
      source === "custom" && saveToHandbook ? cleanedCustom : null,
    );
    setBusy(false);
    if (!outcome.ok) setError(outcome.error);
    else setHandbookError(outcome.handbookError);
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
          <>
            <CustomRoleFields value={custom} onChange={setCustom} autoFocus />
            <label className="-mt-1 flex w-fit items-center gap-2 text-[12.5px]">
              <Checkbox checked={saveToHandbook} onCheckedChange={(checked) => setSaveToHandbook(checked === true)} />
              Also save to the Role Handbook
            </label>
          </>
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
        {handbookError && (
          <div className="flex flex-col gap-1.5">
            <p className="text-[12.5px] leading-snug">
              {name.trim()} was added to the team, but the role wasn't saved to the Role Handbook:
            </p>
            <ErrorLine message={handbookError} />
          </div>
        )}
      </div>

      <DialogFooter>
        {handbookError ? (
          <PressButton type="button" onClick={onClose}>
            Close
          </PressButton>
        ) : (
          <>
            <PressButton type="button" variant="ghost" onClick={onClose}>
              Cancel
            </PressButton>
            <PressButton type="button" disabled={!ready} onClick={() => void submit()}>
              {busy && <LoaderCircle data-icon="inline-start" className="animate-spin" />}
              Add agent
            </PressButton>
          </>
        )}
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
