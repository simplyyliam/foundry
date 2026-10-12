import {
  Check,
  CircleAlert,
  CircleCheck,
  CircleHelp,
  Clock3,
  FolderGit2,
  History,
  LoaderCircle,
  Send,
  ShieldAlert,
  SkipForward,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { AppError, CommitSetupResult, GuidanceAction, HeadlineKind, VerificationGuidance } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { CommitSliceButton, PressButton, type CommitAvailability } from "@/shared";
import { formatWhen, plural } from "../lib/verification";
import { DismissButton, ErrorLine } from "./Notices";
import { SendPanel, type AgentSendHandlers } from "./SendPanel";
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

export interface GuidanceHandlers extends AgentSendHandlers {
  projectId: string;
  sliceId: string;
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
          id={sendAction.id}
          message={sendAction.message ?? ""}
          agentId={sendAction.agentId}
          className="ml-6.5"
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
