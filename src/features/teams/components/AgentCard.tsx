import { Info, Maximize2, ShieldAlert, X } from "lucide-react";
import { useState, type CSSProperties } from "react";
import type { AppError, TeamAgent } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import type { TerminalRegistry } from "../hooks/terminalRegistry";
import type { HooksPreflight } from "../hooks/useHooksPreflight";
import type { StartChoices } from "../hooks/useStartChoices";
import { isLive, type SessionView } from "../lib/session";
import { ErrorLine } from "./ErrorLine";
import { SessionStatus } from "./SessionStatus";
import { StartDialog } from "./StartDialog";
import { StartMenu, type StartChoice } from "./StartMenu";
import { StopControls, type Stopping } from "./StopControls";
import { TerminalSlot } from "./TerminalSlot";
import "./agent-tint.css";

export const HOOKS_SILENT_MESSAGE =
  "Hooks aren't reporting yet. If Claude is asking you to trust this folder, answer it in the terminal. Otherwise write limits may not be enforced for this session.";

export interface AgentCardProps {
  projectId: string;
  agent: TeamAgent;
  avatar: string;
  /** The card colours from the avatar (`tintStyle`); neutral while undefined. */
  tint: CSSProperties | undefined;
  session: SessionView | undefined;
  registry: TerminalRegistry | null;
  selected: boolean;
  onSelect: () => void;
  /** The terminal is in the expanded dialog right now. */
  expanded: boolean;
  onExpand: () => void;
  choices: StartChoices;
  preflight: HooksPreflight;
  onStart: (choice: StartChoice, installedHooks: boolean) => Promise<AppError | null>;
  stopping: Stopping | undefined;
  onStop: (force: boolean) => void;
  /** No `session_started` from the hooks in time. */
  hooksSilent: boolean;
  notice: string | null;
  error: string | null;
  onDismissError: () => void;
}

/**
 * One agent as a live terminal card: avatar, name, title and status on top, the terminal
 * below. Not started or exited, the card keeps its size and tint and shows Start in the middle
 * (over the dimmed last output after an exit, which the expand button shows in full).
 */
export function AgentCard(props: AgentCardProps) {
  const { agent, session, selected, expanded, registry } = props;
  const [choice, setChoice] = useState<StartChoice | null>(null);
  const live = isLive(session);
  const exited = session?.status === "exited";
  const running = session?.status === "running";

  return (
    <section
      aria-label={`${agent.displayName}, ${agent.title}`}
      data-selected={selected || undefined}
      data-hoverable
      style={props.tint}
      onPointerDownCapture={props.onSelect}
      onFocusCapture={props.onSelect}
      className={cn(
        "agent-tint flex aspect-[4/5] max-h-[calc(100vh-8.5rem)] min-h-[420px] flex-col self-start rounded-[20px] p-4 pt-3.5 transition-colors duration-150",
      )}
    >
      <header className="flex h-7 shrink-0 items-center gap-2">
        <img src={props.avatar} alt="" className="size-5 shrink-0 rounded-full object-cover" />
        <h2 className="flex min-w-0 items-baseline gap-1.5 text-[14px]">
          <span className="shrink-0 font-medium text-(--tint-name)">
            {agent.displayName}
          </span>
          <span className="truncate text-(--tint-title)">
            {agent.title}
          </span>
        </h2>
        <SessionStatus session={session} className="ml-1" />
        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          <PressButton
            variant="ghost"
            size="icon-sm"
            aria-label={`Expand ${agent.displayName}'s terminal`}
            title="Expand"
            disabled={!live && !exited}
            onClick={props.onExpand}
            className="size-7 hover:bg-foreground/8"
          >
            <Maximize2 aria-hidden className="size-3.5" />
          </PressButton>
          <StopControls agentName={agent.displayName} running={running} stopping={props.stopping} onStop={props.onStop} />
        </div>
      </header>

      <div className="relative mt-3 min-h-0 flex-1">
        <TerminalSlot
          registry={registry}
          agentId={agent.id}
          active={!expanded}
          live={live}
          className={cn(
            "absolute inset-0 transition-opacity duration-200",
            live ? "opacity-100" : exited ? "opacity-30" : "pointer-events-none opacity-0",
          )}
        />
        {expanded && (
          <p className="absolute inset-0 grid place-items-center text-[12.5px] text-muted-foreground">
            Open in the expanded view
          </p>
        )}
        {!live && !expanded && (
          <div className="pointer-events-none absolute inset-0 grid place-items-center">
            {/* After an exit the panel sits over the dimmed output, so it gets the card's tint behind it. */}
            <div
              className={cn(
                "pointer-events-auto flex flex-col items-center gap-3 rounded-2xl px-6 py-4 text-center",
                exited && "bg-(--tint-surface)/90",
              )}
            >
              <p className="text-[13px] text-(--tint-title)">
                {exited
                  ? session?.exitCode === null
                    ? "Exited"
                    : `Exited with code ${session?.exitCode}`
                  : "Not started"}
              </p>
              <StartMenu
                projectId={props.projectId}
                agentId={agent.id}
                label={exited ? "Start again" : "Start"}
                choices={props.choices}
                onPick={setChoice}
              />
            </div>
          </div>
        )}
      </div>

      {(props.hooksSilent || props.notice || props.error) && (
        <div className="mt-2 flex shrink-0 flex-col gap-1.5">
          {props.hooksSilent && (
            <p role="status" className="flex items-start gap-2 rounded-lg bg-background/55 px-3 py-2 text-[12px] leading-snug">
              <ShieldAlert aria-hidden className="mt-px size-3.5 shrink-0 text-status-starting" />
              {HOOKS_SILENT_MESSAGE}
            </p>
          )}
          {props.notice && (
            <p role="status" className="flex items-center gap-2 rounded-lg bg-background/55 px-3 py-2 text-[12px]">
              <Info aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
              {props.notice}
            </p>
          )}
          {props.error && (
            <ErrorLine message={props.error} className="bg-background/55">
              <PressButton
                variant="ghost"
                size="icon-xs"
                aria-label="Dismiss"
                onClick={props.onDismissError}
                className="-my-0.5 shrink-0"
              >
                <X aria-hidden />
              </PressButton>
            </ErrorLine>
          )}
        </div>
      )}

      <StartDialog
        agent={agent}
        choice={choice}
        onClose={() => setChoice(null)}
        preflight={props.preflight}
        onConfirm={async (picked, installedHooks) => {
          const refusal = await props.onStart(picked, installedHooks);
          if (!refusal) setChoice(null);
          return refusal;
        }}
      />
    </section>
  );
}
