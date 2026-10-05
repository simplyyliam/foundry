import type { Settings } from "@/lib/bindings";
import { useSettingField } from "../../hooks/useSettingField";
import { SettingGroup, SettingRow } from "../SettingRow";
import { SettingSwitch } from "../SettingSwitch";
import { ThemeToggle } from "../ThemeToggle";

export function AppearanceSection({ settings }: { settings: Settings }) {
  const theme = useSettingField(settings, "theme");
  const reduceMotion = useSettingField(settings, "reducedMotionOverride");

  return (
    <SettingGroup>
      <SettingRow
        label="Theme"
        description="System follows your OS and switches with it."
        error={theme.error}
        control={(ids) => <ThemeToggle value={theme.value} onChange={(next) => void theme.commit(next)} ids={ids} />}
      />
      <SettingRow
        label="Reduce motion"
        description="Use fewer animations, even if your system allows them. When off, Foundry follows your system setting."
        error={reduceMotion.error}
        control={(ids) => (
          <SettingSwitch checked={reduceMotion.value} onChange={(next) => void reduceMotion.commit(next)} ids={ids} />
        )}
      />
    </SettingGroup>
  );
}
