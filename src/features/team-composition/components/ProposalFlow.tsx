import { LoaderCircle, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import type { TeamWritten } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { HandbookRoles } from "../hooks/useHandbookRoles";
import type { PlanStatus } from "../hooks/usePlanStatus";
import type { TeamController } from "../hooks/useTeam";
import {
  confirmBlocker,
  draftFromCustom,
  draftFromProposal,
  draftFromRole,
  duplicateNames,
  rosterAgent,
  type DraftAgent,
} from "../lib/draft";
import { AddToProposal } from "./AddToProposal";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorLine } from "./ErrorLine";
import { ProposalCard } from "./ProposalCard";

interface ProposalFlowProps {
  team: TeamController;
  plan: PlanStatus;
  handbook: HandbookRoles;
  /** After `confirmTeam` succeeded (the team has been read again by then). */
  onConfirmed: (written: TeamWritten) => void;
}

/** Why Propose is disabled, or null. Backend messages are shown as-is. */
function proposeBlocker(plan: PlanStatus): string | null {
  if (plan.status === "loading") return "Checking the plan…";
  if (plan.status === "error") return plan.error.message;
  if (plan.slices.length === 0) return "Generate a roadmap first";
  if (plan.summary.confirmed) return null;
  const { unassigned, unconfirmed, acyclic } = plan.summary;
  const parts = [
    unassigned.length > 0 && `${count(unassigned.length, "requirement")} ${unassigned.length === 1 ? "has" : "have"} no slice`,
    unconfirmed.length > 0 && `${count(unconfirmed.length, "requirement")} ${unconfirmed.length === 1 ? "isn't" : "aren't"} confirmed`,
    !acyclic && "the slice dependencies form a cycle",
  ].filter(Boolean);
  return parts.length > 0 ? `Confirm the plan first: ${parts.join(", ")}.` : "Confirm the plan first.";
}

const count = (n: number, noun: string) => `${n} ${noun}${n === 1 ? "" : "s"}`;

/**
 * No team yet: Propose (asks first; it uses the subscription), then edit the proposed cards
 * and confirm. Everything here is local state: leaving the page or reloading drops it.
 */
export function ProposalFlow({ team, plan, handbook, onConfirmed }: ProposalFlowProps) {
  const [drafts, setDrafts] = useState<DraftAgent[] | null>(null);
  const [proposingSince, setProposingSince] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [asking, setAsking] = useState<"propose" | "confirm" | "discard" | null>(null);
  /** The card just added by hand opens in rename mode. */
  const [justAdded, setJustAdded] = useState<string | null>(null);

  const propose = async () => {
    setError(null);
    setProposingSince(Date.now());
    const outcome = await team.compose();
    setProposingSince(null);
    if (outcome.ok) setDrafts(outcome.data.agents.map(draftFromProposal));
    else setError(outcome.error.message);
  };

  const add = (draft: DraftAgent) => {
    setDrafts((current) => [...(current ?? []), draft]);
    setJustAdded(draft.clientId);
  };

  const askDialogs = (
    <>
      <ConfirmDialog
        open={asking === "propose"}
        onOpenChange={(open) => setAsking(open ? "propose" : null)}
        title="Propose a team?"
        description="This asks Claude to propose a team and uses a little of your Claude usage."
        confirmLabel="Propose"
        onConfirm={() => void propose()}
      />
      <ConfirmDialog
        open={asking === "confirm"}
        onOpenChange={(open) => setAsking(open ? "confirm" : null)}
        title="Confirm the team?"
        description="This locks the team. Afterwards you can only add agents."
        confirmLabel="Confirm team"
        onConfirm={async () => {
          const outcome = await team.confirm((drafts ?? []).map(rosterAgent));
          if (!outcome.ok) return outcome.error.message;
          onConfirmed(outcome.data);
          return null;
        }}
      />
      <ConfirmDialog
        open={asking === "discard"}
        onOpenChange={(open) => setAsking(open ? "discard" : null)}
        title="Discard this proposal?"
        description="The cards and your edits are cleared. Proposing again asks Claude again."
        confirmLabel="Discard"
        destructive
        onConfirm={() => {
          setDrafts(null);
          setError(null);
        }}
      />
    </>
  );

  if (drafts === null) {
    const blocker = proposeBlocker(plan);
    const proposing = proposingSince !== null;
    return (
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <PressButton disabled={blocker !== null || proposing} onClick={() => setAsking("propose")}>
            {proposing ? <LoaderCircle data-icon="inline-start" className="animate-spin" /> : <Sparkles data-icon="inline-start" />}
            Propose team
          </PressButton>
          {proposing ? (
            <p role="status" className="text-[12.5px] text-muted-foreground tabular-nums">
              Proposing… <Elapsed since={proposingSince} />
            </p>
          ) : (
            blocker && <p className="min-w-0 flex-1 text-[12.5px] leading-snug text-muted-foreground">{blocker}</p>
          )}
        </div>
        {error && <ErrorLine message={error} />}
        {askDialogs}
      </div>
    );
  }

  const duplicates = duplicateNames(drafts);
  const blocker = confirmBlocker(drafts);
  const slices = plan.status === "ready" ? plan.slices : [];

  return (
    <div className="flex flex-col gap-4">
      {drafts.length > 0 ? (
        <ul aria-label="Proposed agents" className="flex flex-col gap-2.5">
          {drafts.map((draft) => (
            <ProposalCard
              key={draft.clientId}
              draft={draft}
              roles={handbook.roles}
              slices={slices}
              duplicate={duplicates.has(draft.clientId)}
              startEditing={draft.clientId === justAdded}
              onChange={(patch) =>
                setDrafts((current) => current?.map((item) => (item.clientId === draft.clientId ? { ...item, ...patch } : item)) ?? null)
              }
              onRemove={() => setDrafts((current) => current?.filter((item) => item.clientId !== draft.clientId) ?? null)}
            />
          ))}
        </ul>
      ) : (
        <p className="rounded-xl border border-dashed border-border px-4 py-6 text-center text-[12.5px] text-muted-foreground">
          No agents in this proposal. Add them from the handbook or as custom roles.
        </p>
      )}

      <AddToProposal
        handbook={handbook}
        onAddRole={(role) => add(draftFromRole(role))}
        onAddCustom={(role) => add(draftFromCustom(role))}
      />

      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-border/60 pt-5">
        <PressButton disabled={blocker !== null} onClick={() => setAsking("confirm")}>
          Confirm team
        </PressButton>
        <PressButton variant="ghost" onClick={() => setAsking("discard")}>
          Discard proposal
        </PressButton>
        {blocker && <p className="min-w-0 flex-1 text-[12.5px] leading-snug text-muted-foreground">{blocker}</p>}
      </div>
      {askDialogs}
    </div>
  );
}

/** m:ss since `since`, counted locally (there's no progress event). */
function Elapsed({ since }: { since: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const seconds = Math.max(0, Math.floor((now - since) / 1000));
  // Hidden from screen readers: a status that changes every second would be read every second.
  return (
    <span aria-hidden>
      {Math.floor(seconds / 60)}:{String(seconds % 60).padStart(2, "0")}
    </span>
  );
}
