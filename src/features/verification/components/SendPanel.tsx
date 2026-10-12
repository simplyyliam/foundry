import { Check, Copy, LoaderCircle, Send } from "lucide-react";
import { useState } from "react";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { AppError, TeamAgent } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton } from "@/shared";
import type { Outcome } from "../hooks/call";
import type { SessionsLoad } from "../hooks/useAgentSend";
import { useCopy } from "../hooks/useCopy";
import { agentLabel } from "../lib/verification";
import { ErrorLine } from "./Notices";

export interface AgentSendHandlers {
  team: ReadonlyMap<string, TeamAgent>;
  sessions: SessionsLoad;
  isAgentLive: (agentId: string) => boolean;
  send: (agentId: string, message: string) => Promise<Outcome<null>>;
}

interface SendPanelProps extends AgentSendHandlers {
  /** Unique on the page; names the message field. */
  id: string;
  message: string;
  /** Who it goes to; null lets the user pick from the confirmed team. */
  agentId: string | null;
  onCancel: () => void;
  onSent: (name: string) => void;
  className?: string;
}

/**
 * A prepared message, editable, sent to the agent's live session. Nothing is started: with no
 * live session it says so and offers Copy instead.
 */
export function SendPanel({
  id,
  message: prepared,
  agentId: fixedAgentId,
  team,
  sessions,
  isAgentLive,
  send,
  onCancel,
  onSent,
  className,
}: SendPanelProps) {
  const [message, setMessage] = useState(prepared);
  const [agentId, setAgentId] = useState<string | null>(fixedAgentId);
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

  const fieldId = `send-${id}`;
  return (
    <div className={cn("flex flex-col gap-2.5 rounded-lg bg-background/80 p-3 ring-1 ring-foreground/6", className)}>
      {fixedAgentId === null && (
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
          className="max-h-80 min-h-20 bg-background text-[13px]"
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
