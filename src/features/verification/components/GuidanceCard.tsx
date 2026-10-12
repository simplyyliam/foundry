import {
  Check,
  CircleAlert,
  CircleCheck,
  CircleHelp,
  Clock3,
  Copy,
  FolderGit2,
  History,
  LoaderCircle,
  Send,
  ShieldAlert,
  SkipForward,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { AppError, CommitSetupResult, GuidanceAction, HeadlineKind, TeamAgent, VerificationGuidance } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { CommitSliceButton, PressButton, type CommitAvailability } from "@/shared";
import type { Outcome } from "../hooks/call";
import type { SessionsLoad } from "../hooks/useAgentSend";
import { useCopy } from "../hooks/useCopy";
import { agentLabel, formatWhen, plural } from "../lib/verification";
import { DismissButton, ErrorLine } from "./Notices";
import { VerifyButton } from "./VerifyControls";

const HEADLINE: Record<HeadlineKind, { icon: LucideIcon; tone: string; iconTone: string }> = {
  passed: { icon: CircleCheck, tone: "bg-status-running/10", iconTone: "text-status-running" },
  needs_fix: { icon: CircleAlert, tone: "bg-destructive/8", iconTone: "text-destructive" },
  needs_decision: { icon: CircleHelp, tone: "bg-warning/12", iconTone: "text-warning" },
  safe_to_override: { icon: ShieldAlert, tone: "bg-warning/12", iconTone: "text-warning" },
  stale: { icon: History, tone: "bg-muted/70", iconTone: "text-muted-foreground" },
  waiting: { icon: Clock3, tone: "bg-muted/70", iconTone: "text-muted-foreground" },
  skipped: { icon: SkipForward, tone: "bg-warning/12", iconTone: "text-warning" },
};

/** Kinds that only say what to do: their text shows, with no button. */
const TEXT_ONLY: ReadonlySet<GuidanceAction["kind"]> = new Set(["manual", "stop_dev_server"]);

export interface GuidanceHandlers {
  projectId: string;
  sliceId: string;
  team: ReadonlyMap<string, TeamAgent>;
  reportPresent: boolean;
  /** This slice is being verified right now. */
  running: boolean;
  markDone: { run: () => void; busy: boolean; done: boolean };
  commit: { availability: CommitAvailability; open: () => void };
  /** Opens the override dialog (with the guidance's templates). */
  override: () => void;
  commitSetup: {
    run: () => void;
    busy: boolean;
    error: AppError | null;
    result: CommitSetupResult | null;
    dismiss: () => void;
  };
  sessions: SessionsLoad;
  isAgentLive: (agentId: string) => boolean;
  send: (agentId: string, message: string) => Promise<Outcome<null>>;
}

interface GuidanceCardProps extends GuidanceHandlers {
  guidance: VerificationGuidance;
  /** The verification the guidance is about, if there is one. */
  verifiedAt: number | null;
}

/** The verdict: the headline in plain words, and what the user can do about it. */
export function GuidanceCard({ guidance, verifiedAt, ...handlers }: GuidanceCardProps) {
  const { headline, actions } = guidance;
  const { icon: Icon, tone, iconTone } = HEADLINE[headline.kind];
  const [sending, setSending] = useState<string | null>(null);
  const [sent, setSent] = useState<string | null>(null);
  const buttons = actions.filter((action) => !TEXT_ONLY.has(action.kind));
  const steps = actions.filter((action) => TEXT_ONLY.has(action.kind) && action.message);
  const sendAction = actions.find((action) => action.id === sending && action.kind === "send_to_agent") ?? null;

  useEffect(() => {
    if (sent === null) return;
    const timer = setTimeout(() => setSent(null), 6000);
    return () => clearTimeout(timer);
  }, [sent]);

  return (
    <section aria-label="What to do" className={cn("flex flex-col gap-3 rounded-xl px-4 py-3.5", tone)}>
      <div className="flex items-start gap-2.5">
        {headline.kind === "waiting" ? (
          <LoaderCircle aria-hidden className={cn("mt-0.5 size-4 shrink-0 animate-spin motion-reduce:animate-none", iconTone)} />
        ) : (
          <Icon aria-hidden className={cn("mt-0.5 size-4 shrink-0", iconTone)} />
        )}
        <p role="status" className="min-w-0 flex-1 text-[13.5px] leading-snug font-medium text-pretty">
          {headline.text}
        </p>
        {verifiedAt !== null && (
          <time
            dateTime={new Date(verifiedAt).toISOString()}
            title={`Verified ${new Date(verifiedAt).toLocaleString()}`}
            className="mt-px shrink-0 text-[12px] text-muted-foreground tabular-nums"
          >
            {formatWhen(verifiedAt)}
          </time>
        )}
      </div>

      {steps.length > 0 && (
        <ul className="flex flex-col gap-1.5 pl-6.5">
          {steps.map((step) => (
            <li key={step.id} className="text-[12.5px] leading-snug">
              <span className="font-medium">{step.label}</span>
              <span className="text-muted-foreground">: {step.message}</span>
            </li>
          ))}
        </ul>
      )}

      {buttons.length > 0 && (
        <div className="flex flex-wrap gap-2 pl-6.5">
          {buttons.map((action, index) => (
            <ActionButton
              key={action.id}
              action={action}
              primary={index === 0}
              active={sending === action.id}
              onSend={() => {
                setSent(null);
                setSending((current) => (current === action.id ? null : action.id));
              }}
              {...handlers}
            />
          ))}
        </div>
      )}

      {sendAction && (
        <SendPanel
          key={sendAction.id}
          action={sendAction}
          {...handlers}
          onCancel={() => setSending(null)}
          onSent={(name) => {
            setSending(null);
            setSent(name);
          }}
        />
      )}
      {sent && (
        <p role="status" className="flex items-center gap-1.5 pl-6.5 text-[12.5px] text-muted-foreground">
          <Check aria-hidden className="size-3.5 text-status-running" />
          Sent to {sent}.
        </p>
      )}

      <CommitSetupOutcome {...handlers.commitSetup} />
    </section>
  );
}

function ActionButton({
  action,
  primary,
  active,
  onSend,
  projectId,
  sliceId,
  reportPresent,
  running,
  markDone,
  commit,
  override,
  commitSetup,
}: GuidanceHandlers & { action: GuidanceAction; primary: boolean; active: boolean; onSend: () => void }) {
  const variant = primary ? "default" : "secondary";
  switch (action.kind) {
    case "mark_done":
      return (
        <PressButton size="sm" variant={variant} disabled={markDone.busy || markDone.done} onClick={markDone.run}>
          {markDone.busy ? <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" /> : <Check aria-hidden data-icon="inline-start" />}
          {markDone.done ? "Done" : action.label}
        </PressButton>
      );
    case "commit":
      return <CommitSliceButton availability={commit.availability} onOpen={commit.open} variant={variant} label={action.label} />;
    case "reverify":
      return <VerifyButton projectId={projectId} sliceId={sliceId} reportPresent={reportPresent} running={running} again size="sm" />;
    case "override":
      return (
        <PressButton size="sm" variant={variant} onClick={override}>
          <ShieldAlert aria-hidden data-icon="inline-start" />
          {action.label}
        </PressButton>
      );
    case "commit_setup":
      return (
        <PressButton size="sm" variant={variant} disabled={commitSetup.busy} onClick={commitSetup.run}>
          {commitSetup.busy ? (
            <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" />
          ) : (
            <FolderGit2 aria-hidden data-icon="inline-start" />
          )}
          {action.label}
        </PressButton>
      );
    case "send_to_agent":
      return (
        <PressButton size="sm" variant={active ? "outline" : variant} aria-expanded={active} onClick={onSend}>
          <Send aria-hidden data-icon="inline-start" />
          {action.label}
        </PressButton>
      );
    default:
      return null;
  }
}

/**
 * The prepared message, editable, sent to the agent's live session. Nothing is started: with no
 * live session it says so and offers Copy instead.
 */
function SendPanel({
  action,
  team,
  sessions,
  isAgentLive,
  send,
  onCancel,
  onSent,
}: GuidanceHandlers & { action: GuidanceAction; onCancel: () => void; onSent: (name: string) => void }) {
  const [message, setMessage] = useState(action.message ?? "");
  // `null` agent: the slice has several agents, the user picks one.
  const [agentId, setAgentId] = useState<string | null>(action.agentId);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [copyState, copy] = useCopy();
  const name = agentId ? agentLabel(team, agentId).name : null;
  const live = agentId !== null && isAgentLive(agentId);
  // Unknown sessions (the list couldn't be read) don't block: the backend refuses a dead one itself.
  const known = sessions.status === "ready";
  const empty = message.trim() === "";
  const canSend = agentId !== null && !empty && !busy && (live || !known);

  const submit = async () => {
    if (!canSend || agentId === null) return;
    setBusy(true);
    setError(null);
    const outcome = await send(agentId, message);
    setBusy(false);
    if (outcome.ok) onSent(name ?? agentId);
    else setError(outcome.error);
  };

  const fieldId = `send-${action.id}`;
  return (
    <div className="ml-6.5 flex flex-col gap-2.5 rounded-lg bg-background/80 p-3 ring-1 ring-foreground/6">
      {action.agentId === null && (
        <div className="flex flex-col gap-1.5">
          <Label className="text-[12.5px]">Send to</Label>
          {team.size === 0 ? (
            <p className="text-[12.5px] text-muted-foreground">No confirmed team to pick from. Copy the message instead.</p>
          ) : (
            <Select
              value={agentId}
              items={[...team.values()].map((agent) => ({ value: agent.id, label: agent.displayName }))}
              onValueChange={(next) => {
                if (typeof next === "string") setAgentId(next);
              }}
            >
              <SelectTrigger size="sm" aria-label="Agent" className="w-56 bg-background text-[13px]">
                <SelectValue>
                  {() => (name ? <span className="truncate">{name}</span> : <span className="text-muted-foreground">Pick an agent</span>)}
                </SelectValue>
              </SelectTrigger>
              <SelectContent alignItemWithTrigger={false} className="min-w-56">
                {[...team.values()].map((agent) => (
                  <SelectItem key={agent.id} value={agent.id} className="text-[13px]">
                    <span className="min-w-0 truncate">{agent.displayName}</span>
                    <span className="ml-auto truncate text-[12px] text-muted-foreground">{agent.title}</span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        </div>
      )}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={fieldId} className="text-[12.5px]">
          {name ? `Message to ${name}` : "Message"}
        </Label>
        <Textarea
          id={fieldId}
          autoFocus
          value={message}
          onChange={(event) => setMessage(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              void submit();
            } else if (event.key === "Escape") {
              event.stopPropagation();
              onCancel();
            }
          }}
          className="min-h-20 bg-background text-[13px]"
        />
      </div>
      {agentId !== null && known && !live && (
        <p role="status" className="text-[12.5px] leading-snug text-muted-foreground">
          {name} has no live session, so nothing can be sent. Copy the message and give it to {name} when it runs.
        </p>
      )}
      {sessions.status === "loading" && agentId !== null && (
        <p className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground">
          <LoaderCircle aria-hidden className="size-3.5 animate-spin" />
          Checking {name}'s session…
        </p>
      )}
      {error && <ErrorLine message={error.message} />}
      {copyState === "failed" && (
        <p className="text-[12.5px] text-muted-foreground">Couldn't copy to the clipboard. Select the text above instead.</p>
      )}
      <div className="flex flex-wrap items-center justify-end gap-2">
        <PressButton type="button" size="sm" variant="ghost" onClick={onCancel}>
          Cancel
        </PressButton>
        <PressButton type="button" size="sm" variant="secondary" disabled={empty} onClick={() => void copy(message)}>
          {copyState === "copied" ? <Check aria-hidden data-icon="inline-start" /> : <Copy aria-hidden data-icon="inline-start" />}
          {copyState === "copied" ? "Copied" : "Copy"}
        </PressButton>
        <PressButton type="button" size="sm" disabled={!canSend} onClick={() => void submit()}>
          {busy ? <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" /> : <Send aria-hidden data-icon="inline-start" />}
          {name ? `Send to ${name}` : "Send"}
        </PressButton>
      </div>
    </div>
  );
}

function CommitSetupOutcome({ error, result, dismiss }: GuidanceHandlers["commitSetup"]) {
  if (error) {
    return (
      <ErrorLine message={error.message} className="ml-6.5">
        <DismissButton onClick={dismiss} />
      </ErrorLine>
    );
  }
  if (!result) return null;
  return (
    <div role="status" className="ml-6.5 flex items-start gap-2 rounded-lg bg-background/70 py-1.5 pr-1.5 pl-3 text-[12.5px]">
      <p className="min-w-0 flex-1 py-px leading-snug">
        {result.files.length === 0
          ? "Nothing needed committing."
          : `Committed ${plural(result.files.length, "setup file")} as ${result.commit.slice(0, 7)}.`}
      </p>
      <DismissButton onClick={dismiss} />
    </div>
  );
}
