import { Badge } from "@/components/ui/badge";

/** Marks a setting that's stored but not used by anything yet. Remove it once the feature lands. */
export function NotActiveTag() {
  return (
    <Badge variant="outline" className="h-4.5 rounded-md border-border/70 px-1.5 text-[10.5px] font-medium text-muted-foreground">
      Not active yet
    </Badge>
  );
}
