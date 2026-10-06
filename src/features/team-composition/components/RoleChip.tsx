import { Badge } from "@/components/ui/badge";
import type { HandbookRole } from "@/lib/bindings";

/**
 * Where an agent comes from: the handbook role's title, or "Custom". A role that's no longer
 * in the handbook shows its id.
 */
export function RoleChip({ handbookRoleId, roles }: { handbookRoleId: string | null; roles: readonly HandbookRole[] | null }) {
  if (handbookRoleId === null) {
    return (
      <Badge variant="outline" className="h-[18px] px-1.5 text-[11px] font-normal text-muted-foreground">
        Custom
      </Badge>
    );
  }
  const title = roles?.find((role) => role.id === handbookRoleId)?.title ?? handbookRoleId;
  return (
    <Badge variant="secondary" className="h-[18px] max-w-48 px-1.5 text-[11px] font-normal" title={`From the Role Handbook: ${title}`}>
      <span className="truncate">{title}</span>
    </Badge>
  );
}
