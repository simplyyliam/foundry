import { useState } from "react";
import type { Settings } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useSettingField } from "../../hooks/useSettingField";
import { CommitInput } from "../CommitInput";
import { ProjectsFolderDialog } from "../ProjectsFolderDialog";
import { SettingGroup, SettingRow } from "../SettingRow";

export function GeneralSection({ settings }: { settings: Settings }) {
  const projectsDir = useSettingField(settings, "defaultProjectsDir");
  const maxAgents = useSettingField(settings, "maxConcurrentAgents");
  const claudePath = useSettingField(settings, "claudePath");
  const [picking, setPicking] = useState(false);

  return (
    <SettingGroup>
      <SettingRow
        label="Default projects folder"
        description={
          projectsDir.value ? (
            // Long paths keep their end (the folder name) visible.
            <span dir="rtl" title={projectsDir.value} className="block max-w-[22rem] truncate text-left">
              <bdi dir="ltr">{projectsDir.value}</bdi>
            </span>
          ) : (
            "Not set. New projects go wherever you pick when adding one."
          )
        }
        error={projectsDir.error}
        control={
          <>
            <PressButton size="sm" variant="outline" className="h-8 rounded-md" onClick={() => setPicking(true)}>
              Change
            </PressButton>
            <PressButton
              size="sm"
              variant="ghost"
              className="h-8 rounded-md"
              disabled={projectsDir.value === null}
              onClick={() => void projectsDir.commit(null)}
            >
              Clear
            </PressButton>
          </>
        }
      />
      <ProjectsFolderDialog
        open={picking}
        onOpenChange={setPicking}
        initialPath={projectsDir.value}
        onSelect={(path) => {
          setPicking(false);
          void projectsDir.commit(path);
        }}
      />

      <SettingRow
        label="Max concurrent agents"
        description="Each running agent uses your Claude usage."
        error={maxAgents.error}
        control={(ids) => (
          <CommitInput
            type="number"
            inputMode="numeric"
            min={1}
            step={1}
            value={String(maxAgents.value)}
            onCommit={(text) => {
              // An emptied field just goes back to the saved number.
              if (text.trim() === "") return;
              const next = Number(text);
              if (Number.isFinite(next)) void maxAgents.commit(next);
            }}
            aria-labelledby={ids.labelId}
            aria-describedby={ids.describedBy}
            className="h-8 w-20 rounded-md text-right tabular-nums"
          />
        )}
      />

      <SettingRow
        label="Claude path"
        description="Leave empty to find Claude Code automatically"
        error={claudePath.error}
        control={(ids) => (
          <>
            <CommitInput
              value={claudePath.value ?? ""}
              placeholder="Automatic"
              onCommit={(text) => void claudePath.commit(text.trim() === "" ? null : text.trim())}
              aria-labelledby={ids.labelId}
              aria-describedby={ids.describedBy}
              className="h-8 w-64 rounded-md"
            />
            <PressButton
              size="sm"
              variant="ghost"
              className="h-8 rounded-md"
              aria-label="Clear Claude path"
              disabled={claudePath.value === null}
              onClick={() => void claudePath.commit(null)}
            >
              Clear
            </PressButton>
          </>
        )}
      />

      <SettingRow
        label="Default AGENTS.md template"
        control={<span className="font-mono text-[12.5px] text-muted-foreground">{settings.agentsTemplateId}</span>}
      />
    </SettingGroup>
  );
}
