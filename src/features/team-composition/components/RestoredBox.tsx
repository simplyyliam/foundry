import { ShieldCheck } from "lucide-react";
import type { FloorRestored, TeamAgent } from "@/lib/bindings";

/**
 * `floorRestored` from confirming (or adding): protected patterns that were missing from what
 * was asked for and that the backend put back, per agent.
 */
export function RestoredBox({ restored, agents }: { restored: FloorRestored[]; agents: readonly TeamAgent[] }) {
  const nameOf = (agentId: string) => agents.find((agent) => agent.id === agentId)?.displayName ?? agentId;
  return (
    <div role="status" className="flex flex-col gap-2 rounded-xl bg-muted/60 p-4 text-[13px] ring-1 ring-foreground/5">
      <p className="flex items-center gap-2 font-medium">
        <ShieldCheck aria-hidden className="size-4 text-primary" />
        These protections were added back:
      </p>
      <ul className="flex flex-col gap-1.5 text-[12.5px]">
        {restored.map((entry) => (
          <li key={entry.agentId} className="flex flex-wrap items-baseline gap-x-1.5 gap-y-1">
            <span className="text-muted-foreground">{nameOf(entry.agentId)}:</span>
            {entry.globs.map((glob) => (
              <code key={glob} className="rounded bg-background px-1 py-px font-mono text-[12px] ring-1 ring-foreground/10">
                {glob}
              </code>
            ))}
          </li>
        ))}
      </ul>
    </div>
  );
}
