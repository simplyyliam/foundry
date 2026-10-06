import { ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import type { AvatarChoice, HandbookRole, TeamAgent } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { AvatarPopover } from "./AvatarPopover";
import { RoleChip } from "./RoleChip";

interface AgentCardProps {
  agent: TeamAgent;
  avatarSrc: string;
  roles: readonly HandbookRole[] | null;
  onChooseAvatar: (choice: AvatarChoice) => Promise<string | null>;
}

/** One confirmed agent: avatar (click to change), name, title, role, and its permissions. */
export function AgentCard({ agent, avatarSrc, roles, onChooseAvatar }: AgentCardProps) {
  return (
    <li className="flex gap-3 rounded-xl bg-card p-4 ring-1 ring-foreground/8">
      <AvatarPopover name={agent.displayName} src={avatarSrc} onChoose={onChooseAvatar} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <div className="flex min-w-0 flex-col">
          <p className="truncate text-[13.5px] font-medium">{agent.displayName}</p>
          <p className="truncate text-[12.5px] text-muted-foreground">{agent.title}</p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <RoleChip handbookRoleId={agent.sourceHandbookRoleId} roles={roles} />
        </div>
        <Collapsible className="group/permissions">
          <CollapsibleTrigger
            render={
              <PressButton
                variant="ghost"
                size="xs"
                pressScale={0.97}
                className="-ml-2 h-6 gap-1 rounded-md px-2 text-[12px] text-muted-foreground"
              />
            }
          >
            <ChevronRight
              data-icon="inline-start"
              className="transition-transform duration-150 group-data-open/permissions:rotate-90 motion-reduce:transition-none"
            />
            Permissions
          </CollapsibleTrigger>
          <CollapsibleContent>
            <dl className="mt-1.5 flex flex-col gap-2 rounded-lg bg-muted/50 p-3 text-[12px]">
              <GlobList term="Can write" globs={agent.writeGlobs} />
              <GlobList term="Protected (never written)" globs={agent.protectedGlobs} />
            </dl>
          </CollapsibleContent>
        </Collapsible>
      </div>
    </li>
  );
}

function GlobList({ term, globs }: { term: string; globs: string[] }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-muted-foreground">{term}</dt>
      <dd className="flex flex-wrap gap-1">
        {globs.length === 0 ? (
          <span className="text-muted-foreground italic">None</span>
        ) : (
          globs.map((glob) => (
            <code key={glob} className="rounded bg-background px-1 py-px font-mono text-[11.5px] ring-1 ring-foreground/10">
              {glob}
            </code>
          ))
        )}
      </dd>
    </div>
  );
}
