import type { NotificationSeverity, Settings } from "@/lib/bindings";
import { useSettingField } from "../../hooks/useSettingField";
import { NotActiveTag } from "../NotActiveTag";
import { SettingGroup, SettingRow } from "../SettingRow";
import { SettingSelect } from "../SettingSelect";
import { SettingSwitch } from "../SettingSwitch";

const SEVERITIES: readonly { value: NotificationSeverity; label: string }[] = [
  { value: "danger", label: "Danger" },
  { value: "warn_and_danger", label: "Warn and danger" },
  { value: "all", label: "All" },
];

export function NotificationsSection({ settings }: { settings: Settings }) {
  const enabled = useSettingField(settings, "osNotificationsEnabled");
  const severity = useSettingField(settings, "osNotificationSeverity");

  return (
    <SettingGroup>
      <SettingRow
        label="Desktop notifications"
        tag={<NotActiveTag />}
        error={enabled.error}
        control={(ids) => <SettingSwitch checked={enabled.value} onChange={(next) => void enabled.commit(next)} ids={ids} />}
      />
      <SettingRow
        label="Minimum severity"
        description="Notify only at this level or above."
        tag={<NotActiveTag />}
        error={severity.error}
        control={(ids) => (
          <SettingSelect value={severity.value} options={SEVERITIES} onChange={(next) => void severity.commit(next)} ids={ids} />
        )}
      />
    </SettingGroup>
  );
}
