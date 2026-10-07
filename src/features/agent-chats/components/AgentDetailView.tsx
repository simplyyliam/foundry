import { X } from "lucide-react";
import type { KeyboardEvent, ReactNode } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { TeamAgent } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton, usePressScale } from "@/shared";
import type { AgentDetail } from "../hooks/useAgentDetail";
import { TAB_LABELS, type DetailTab } from "../lib/events";
import { ActivityTab } from "./ActivityTab";
import { DangerTab } from "./DangerTab";
import { DetailHeader } from "./DetailHeader";
import { ErrorLine } from "./ErrorLine";
import { ImplementationsTab } from "./ImplementationsTab";

interface AgentDetailViewProps {
  detail: AgentDetail;
  agent: TeamAgent;
  avatar: string;
  now: number;
  /** The panel's back button (Escape does the same). The pop-out has none. */
  onBack?: () => void;
  /** Header actions: the pop-out button, or the pop-out's close button. */
  trailing?: ReactNode;
  className?: string;
}

const TABS: readonly DetailTab[] = ["implementations", "activity", "danger"];

/** One agent: header, the three tabs, and the selected tab's events. */
export function AgentDetailView({ detail, agent, avatar, now, onBack, trailing, className }: AgentDetailViewProps) {
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape" && onBack && !event.defaultPrevented) {
      event.preventDefault();
      onBack();
    }
  };

  return (
    <Tabs
      value={detail.tab}
      onValueChange={(value: DetailTab) => detail.setTab(value)}
      className={cn("flex min-h-0 flex-1 flex-col gap-0", className)}
      onKeyDown={onKeyDown}
    >
      <div className="flex flex-col gap-3 p-2 pb-1">
        <DetailHeader agent={agent} avatar={avatar} onBack={onBack} trailing={trailing} />
        <TabsList aria-label={`${agent.displayName}'s events`} className="h-auto! gap-1 rounded-none bg-transparent p-0 px-1.5">
          {TABS.map((tab) => (
            <PillTab key={tab} value={tab} dot={tab === "danger" ? detail.pendingAlert : null} />
          ))}
        </TabsList>
      </div>

      {(detail.unreadError || detail.markError) && (
        <div className="flex flex-col gap-1.5 px-4 pt-2">
          {detail.unreadError && <ErrorLine message={detail.unreadError} />}
          {detail.markError && (
            <ErrorLine message={detail.markError}>
              <PressButton
                type="button"
                size="icon-xs"
                variant="ghost"
                aria-label="Dismiss"
                className="rounded-md"
                onClick={detail.dismissMarkError}
              >
                <X />
              </PressButton>
            </ErrorLine>
          )}
        </div>
      )}

      {detail.tab && (
        <>
          <TabsContent value="implementations" className="min-h-0">
            <ImplementationsTab
              stream={detail.all}
              avatar={avatar}
              onLoadOlder={() => detail.loadOlder("implementations")}
              onRetry={detail.retry}
            />
          </TabsContent>
          <TabsContent value="activity" className="min-h-0">
            <ActivityTab stream={detail.all} now={now} onLoadOlder={() => detail.loadOlder("activity")} onRetry={detail.retry} />
          </TabsContent>
          <TabsContent value="danger" className="min-h-0">
            <DangerTab stream={detail.alerts} onLoadOlder={() => detail.loadOlder("danger")} onRetry={detail.retry} />
          </TabsContent>
        </>
      )}
    </Tabs>
  );
}

const DOT_LABEL = { danger: "unread danger", warn: "unread warning" } as const;

function PillTab({ value, dot }: { value: DetailTab; dot: AgentDetail["pendingAlert"] }) {
  const ref = usePressScale<HTMLButtonElement>({ scale: 0.96 });
  return (
    <TabsTrigger
      ref={ref}
      value={value}
      className="h-[26px] flex-none rounded-[10px] bg-pill px-2 text-[11.5px] font-normal text-foreground hover:bg-pill-hover data-active:bg-pill-active dark:text-foreground dark:data-active:border-transparent dark:data-active:bg-pill-active"
    >
      {TAB_LABELS[value]}
      {dot && (
        <>
          <span
            aria-hidden
            className={cn(
              "absolute -top-0.5 -right-0.5 size-[7px] rounded-full ring-[1.5px] ring-sidebar",
              dot === "danger" ? "bg-destructive" : "bg-warning",
            )}
          />
          <span className="sr-only">({DOT_LABEL[dot]})</span>
        </>
      )}
    </TabsTrigger>
  );
}
