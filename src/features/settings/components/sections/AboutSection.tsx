import { LoaderCircle } from "lucide-react";
import { useAppInfo } from "../../hooks/useAppInfo";
import { SettingGroup, SettingRow } from "../SettingRow";

export function AboutSection() {
  const info = useAppInfo();

  if (!info) {
    return (
      <div className="flex justify-center py-10">
        <LoaderCircle aria-label="Loading" className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (!info.ok) {
    return (
      <p role="alert" className="py-4 text-[12px] text-destructive">
        {info.error.message}
      </p>
    );
  }
  return (
    <SettingGroup>
      <SettingRow label="Version" control={<Value>{info.data.version}</Value>} />
      <SettingRow label="OS" control={<Value>{info.data.os}</Value>} />
    </SettingGroup>
  );
}

function Value({ children }: { children: string }) {
  return <span className="text-[13px] text-muted-foreground tabular-nums select-text">{children}</span>;
}
