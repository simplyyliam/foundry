import type { Settings, VerifyMode } from "@/lib/bindings";
import { useSettingField } from "../../hooks/useSettingField";
import { NotActiveTag } from "../NotActiveTag";
import { SettingGroup, SettingRow } from "../SettingRow";
import { SettingSelect } from "../SettingSelect";
import { SettingSwitch } from "../SettingSwitch";

const VERIFY_MODES: readonly { value: VerifyMode; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "manual", label: "Manual" },
];

export function AgentsSection({ settings }: { settings: Settings }) {
  const autoHooks = useSettingField(settings, "autoInstallHooks");
  const verifyMode = useSettingField(settings, "defaultVerifyMode");

  return (
    <SettingGroup>
      <SettingRow
        label="Auto-install hooks"
        description="Install Foundry's hooks before an agent starts, if the project doesn't have them."
        error={autoHooks.error}
        control={(ids) => (
          <SettingSwitch checked={autoHooks.value} onChange={(next) => void autoHooks.commit(next)} ids={ids} />
        )}
      />
      <SettingRow
        label="Default verify mode"
        tag={<NotActiveTag />}
        error={verifyMode.error}
        control={(ids) => (
          <SettingSelect value={verifyMode.value} options={VERIFY_MODES} onChange={(next) => void verifyMode.commit(next)} ids={ids} />
        )}
      />
    </SettingGroup>
  );
}
