import { Check, Copy, Send } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { Badge } from "@/components/ui/badge";
import type { FixPrompt } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useCopy } from "../hooks/useCopy";
import type { Loaded } from "../hooks/useSliceDetail";
import { agentLabel, plural } from "../lib/verification";
import { ErrorLine } from "./Notices";
import { SendPanel, type AgentSendHandlers } from "./SendPanel";
import { Loading, Muted } from "./SheetParts";

interface FixPromptsSectionProps extends AgentSendHandlers {
  load: Loaded<FixPrompt[]>;
}

/** One card per agent with the prompt the backend prepared for their bugs: Copy, or send it (editable first). */
export function FixPromptsSection({ load, ...handlers }: FixPromptsSectionProps) {
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2.5">
      <h3 id={headingId} className="text-[13.5px] font-semibold">
        Fix prompts
      </h3>
      {load.status === "loading" && <Loading label="Preparing the fix prompts" />}
      {load.status === "error" && <ErrorLine message={load.error.message} />}
      {load.status === "ready" && load.value.length === 0 && <Muted>No fix prompts for these bugs.</Muted>}
      {load.status === "ready" && load.value.length > 0 && (
        <ul className="flex flex-col gap-2.5">
          {load.value.map((prompt) => (
            <li key={prompt.agentId}>
              {/* Keyed by the text, so a regenerated prompt resets an open, edited send box. */}
              <FixPromptCard key={prompt.prompt} prompt={prompt} {...handlers} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function FixPromptCard({ prompt, ...handlers }: AgentSendHandlers & { prompt: FixPrompt }) {
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState<string | null>(null);
  const [copyState, copy] = useCopy();
  const role = agentLabel(handlers.team, prompt.agentId).role;

  useEffect(() => {
    if (sent === null) return;
    const timer = setTimeout(() => setSent(null), 6000);
    return () => clearTimeout(timer);
  }, [sent]);

  return (
    <article aria-label={`Fix prompt for ${prompt.agentName}`} className="flex flex-col gap-2.5 rounded-xl bg-muted/40 px-3.5 py-3 ring-1 ring-foreground/6">
      <div className="flex items-center gap-2">
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-[13px] font-medium">{prompt.agentName}</span>
          {role && <span className="truncate text-[11.5px] text-muted-foreground">{role}</span>}
        </div>
        <Badge variant="outline" className="h-5 shrink-0 px-2 text-[11px] tabular-nums">
          {plural(prompt.bugIds.length, "bug")}
        </Badge>
      </div>

      {sending ? (
        <SendPanel
          id={`fix-${prompt.agentId}`}
          message={prompt.prompt}
          agentId={prompt.agentId}
          {...handlers}
          onCancel={() => setSending(false)}
          onSent={(name) => {
            setSending(false);
            setSent(name);
          }}
        />
      ) : (
        <>
          <pre
            tabIndex={0}
            aria-label={`Prompt for ${prompt.agentName}`}
            className="max-h-64 overflow-auto rounded-lg bg-background/80 px-3 py-2 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap break-words ring-1 ring-foreground/6 outline-none select-text focus-visible:ring-2 focus-visible:ring-ring/40"
          >
            {prompt.prompt}
          </pre>
          {copyState === "failed" && (
            <p className="text-[12.5px] text-muted-foreground">Couldn't copy to the clipboard. Select the text above instead.</p>
          )}
          <div className="flex flex-wrap items-center justify-end gap-2">
            <PressButton type="button" size="sm" variant="secondary" onClick={() => void copy(prompt.prompt)}>
              {copyState === "copied" ? <Check aria-hidden data-icon="inline-start" /> : <Copy aria-hidden data-icon="inline-start" />}
              {copyState === "copied" ? "Copied" : "Copy"}
            </PressButton>
            <PressButton
              type="button"
              size="sm"
              onClick={() => {
                setSent(null);
                setSending(true);
              }}
            >
              <Send aria-hidden data-icon="inline-start" />
              Send to {prompt.agentName}
            </PressButton>
          </div>
        </>
      )}
      {sent && (
        <p role="status" className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground">
          <Check aria-hidden className="size-3.5 text-status-running" />
          Sent to {sent}.
        </p>
      )}
    </article>
  );
}
