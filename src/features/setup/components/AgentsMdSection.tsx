import { LoaderCircle, TriangleAlert } from "lucide-react";
import { useRef, useState, type ReactNode } from "react";
import type { GenerateAgentsResult } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAgentsSetup } from "../hooks/useAgentsSetup";
import { DesignFamilyBlock } from "./DesignFamilyBlock";
import { GenerateAgentsBlock } from "./GenerateAgentsBlock";
import { PlaceholderRow } from "./PlaceholderRow";
import { SetupSection } from "./SetupSection";

/**
 * The AGENTS.md section: confirm every template placeholder and the design family, then
 * generate the file. Drafts live here (not in the rows) so Generate knows when something is
 * being edited; they're lost on reload, which shows the last confirmed values.
 */
export function AgentsMdSection({ projectId }: { projectId: string }) {
  const { load, folderMissing, reload, confirm, generate } = useAgentsSetup(projectId);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [familyDraft, setFamilyDraft] = useState<string[] | null>(null);
  const [result, setResult] = useState<GenerateAgentsResult | null>(null);
  const listRef = useRef<HTMLOListElement>(null);
  const disabled = folderMissing !== null;

  const setDraft = (key: string, text: string | null) =>
    setDrafts((current) => {
      const next = { ...current };
      if (text === null) delete next[key];
      else next[key] = text;
      return next;
    });

  const intro = "AGENTS.md is the rulebook every agent reads at the start of each session. Confirm each value, then generate it.";

  if (load.status !== "ready") {
    return (
      <SetupSection title="AGENTS.md" intro={intro}>
        {load.status === "loading" ? (
          <div className="flex justify-center py-10">
            <LoaderCircle aria-label="Loading the AGENTS.md placeholders" className="size-4 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <ErrorLine message={load.error.message}>
            <PressButton size="xs" variant="outline" className="rounded-md" onClick={() => void reload()}>
              Try again
            </PressButton>
          </ErrorLine>
        )}
      </SetupSection>
    );
  }

  const placeholders = load.placeholders.filter((placeholder) => placeholder.inTemplate);
  const family = load.placeholders.find((placeholder) => !placeholder.inTemplate) ?? null;
  const all = family ? [...placeholders, family] : placeholders;
  const isPending = (key: string, confirmed: boolean) =>
    !confirmed || (family?.key === key ? familyDraft !== null : key in drafts);
  const pending = all.filter((placeholder) => isPending(placeholder.key, placeholder.confirmed));
  const confirmedCount = all.length - pending.length;

  /** Confirms, drops the draft, and moves to the next placeholder still waiting. */
  const confirmText = async (key: string, text: string) => {
    const outcome = await confirm(key, text);
    if (!outcome.ok) return outcome;
    setDraft(key, null);
    setResult(null);
    const after = placeholders.slice(placeholders.findIndex((placeholder) => placeholder.key === key) + 1);
    const next = after.find((placeholder) => isPending(placeholder.key, placeholder.confirmed));
    if (next) {
      // After the list re-renders with this one confirmed.
      requestAnimationFrame(() =>
        listRef.current?.querySelector<HTMLElement>(`[data-placeholder-input="${CSS.escape(next.key)}"]`)?.focus(),
      );
    }
    return outcome;
  };

  return (
    <SetupSection title="AGENTS.md" intro={intro}>
      {folderMissing && <ErrorLine message={folderMissing} />}

      <p className="text-[12.5px] text-muted-foreground tabular-nums" aria-live="polite">
        {confirmedCount} of {all.length} confirmed
      </p>

      <ol ref={listRef} className="flex flex-col" aria-label="AGENTS.md placeholders">
        {placeholders.map((placeholder) => (
          <PlaceholderRow
            key={placeholder.key}
            placeholder={placeholder}
            draft={drafts[placeholder.key] ?? null}
            onDraftChange={(text) => setDraft(placeholder.key, text)}
            onConfirm={(text) => confirmText(placeholder.key, text)}
            disabled={disabled}
          />
        ))}
      </ol>

      {family && (
        <DesignFamilyBlock
          placeholder={family}
          draft={familyDraft}
          onDraftChange={setFamilyDraft}
          onConfirm={async (tags) => {
            const outcome = await confirm(family.key, tags.join(", "));
            if (outcome.ok) {
              setFamilyDraft(null);
              setResult(null);
            }
            return outcome;
          }}
          disabled={disabled}
        />
      )}

      <GenerateAgentsBlock
        generate={generate}
        pending={pending.map((placeholder) => placeholder.label)}
        disabled={disabled}
        result={result}
        onResult={setResult}
      />
    </SetupSection>
  );
}

function ErrorLine({ message, children }: { message: string; children?: ReactNode }) {
  return (
    <div role="alert" className="flex items-start gap-2 rounded-lg bg-destructive/8 py-2 pr-1.5 pl-3 text-[12.5px]">
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 leading-snug">{message}</p>
      {children}
    </div>
  );
}
