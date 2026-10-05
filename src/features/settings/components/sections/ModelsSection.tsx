import { LoaderCircle, RotateCw } from "lucide-react";
import type { ReactNode } from "react";
import type { ClaudeStatus, Settings } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useClaudeCodeStatus } from "../../hooks/useClaudeCodeStatus";
import { useSettingField } from "../../hooks/useSettingField";
import { CommitInput } from "../CommitInput";
import { SettingGroup, SettingRow } from "../SettingRow";

const MS_PER_MINUTE = 60_000;

/** 300000 → "5", 90000 → "1.5". */
function toMinutes(ms: number): string {
  return String(Number((ms / MS_PER_MINUTE).toFixed(2)));
}

export function ModelsSection({ settings }: { settings: Settings }) {
  const model = useSettingField(settings, "defaultModel");
  const timeout = useSettingField(settings, "generationTimeoutMs");

  return (
    <>
      <SettingGroup>
        <SettingRow
          label="Default model"
          description="An alias like sonnet or opus, or a full model name."
          error={model.error}
          control={(ids) => (
            <CommitInput
              value={model.value ?? ""}
              placeholder="Claude Code default"
              onCommit={(text) => void model.commit(text.trim() === "" ? null : text.trim())}
              aria-labelledby={ids.labelId}
              aria-describedby={ids.describedBy}
              className="h-8 w-56 rounded-md"
            />
          )}
        />
        <SettingRow
          label="Roadmap generation timeout"
          description="How long one roadmap generation may run."
          error={timeout.error}
          control={(ids) => (
            <>
              <CommitInput
                type="number"
                inputMode="decimal"
                min={0}
                step="any"
                value={toMinutes(timeout.value)}
                onCommit={(text) => {
                  if (text.trim() === "") return;
                  const minutes = Number(text);
                  // Zero or less is the backend's to refuse; its message shows under the row.
                  if (Number.isFinite(minutes)) void timeout.commit(Math.round(minutes * MS_PER_MINUTE));
                }}
                aria-labelledby={ids.labelId}
                aria-describedby={ids.describedBy}
                className="h-8 w-20 rounded-md text-right tabular-nums"
              />
              <span aria-hidden className="text-[12.5px] text-muted-foreground">
                min
              </span>
            </>
          )}
        />
      </SettingGroup>
      <ClaudeCodeStatus />
    </>
  );
}

/** What `claudeStatus` says, word for word. Never the account (email, organisation, plan). */
function ClaudeCodeStatus() {
  const { state, refresh } = useClaudeCodeStatus();
  const checking = state.status === "checking";
  const data: ClaudeStatus | null = state.status === "ready" ? state.data : state.status === "checking" ? state.last : null;

  return (
    <SettingGroup
      title="Claude Code"
      action={
        <PressButton size="sm" variant="ghost" className="h-7 rounded-md text-[12.5px]" disabled={checking} onClick={refresh}>
          {checking ? <LoaderCircle className="animate-spin" /> : <RotateCw />}
          Refresh
        </PressButton>
      }
    >
      {state.status === "error" ? (
        <p role="alert" className="py-3 text-[12px] text-destructive">
          {state.message}
        </p>
      ) : !data ? (
        <div className="flex justify-center py-6">
          <LoaderCircle aria-label="Checking Claude Code" className="size-4 animate-spin text-muted-foreground" />
        </div>
      ) : (
        <div aria-busy={checking || undefined} className="flex flex-col">
          <SettingRow
            label="Installed"
            description={
              data.path ? (
                <span dir="rtl" title={data.path} className="block max-w-[22rem] truncate text-left">
                  <bdi dir="ltr">{data.path}</bdi>
                </span>
              ) : undefined
            }
            control={
              <Value>
                {data.path ? "Found" : "Not found"}
                {data.version && <span className="text-muted-foreground"> · {data.version}</span>}
              </Value>
            }
          />
          <SettingRow label="Signed in" control={<Value>{data.loggedIn ? "Signed in" : "Not signed in"}</Value>} />
          <SettingRow label="Auth mode" control={<Value>{data.authMethod ?? "—"}</Value>} />
          {data.problem && (
            <p role="status" className="pt-1 pb-3 text-[12px] text-destructive">
              {data.problem.message}
            </p>
          )}
        </div>
      )}
    </SettingGroup>
  );
}

function Value({ children }: { children: ReactNode }) {
  return <span className="text-[13px] select-text">{children}</span>;
}
