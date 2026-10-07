import { useCallback, useEffect, useRef, useState } from "react";
import type { AppError, TeamAgent } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAgentAvatars } from "../hooks/useAgentAvatars";
import { useAvatarTints } from "../hooks/useAvatarTints";
import { useHooksPreflight } from "../hooks/useHooksPreflight";
import { useHooksWatch } from "../hooks/useHooksWatch";
import { useSessions } from "../hooks/useSessions";
import { useStartChoices } from "../hooks/useStartChoices";
import { useTerminals } from "../hooks/useTerminals";
import { isLive } from "../lib/session";
import { AgentCard } from "./AgentCard";
import { ErrorLine } from "./ErrorLine";
import { ExpandedTerminal } from "./ExpandedTerminal";
import type { StartChoice } from "./StartMenu";
import type { Stopping } from "./StopControls";

const NOTICE_MS = 6000;

/** The confirmed team as live terminal cards, side by side, wrapping at ~420 px per card. */
export function TeamBoard({ projectId, agents }: { projectId: string; agents: TeamAgent[] }) {
  const sessions = useSessions(projectId);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [notices, setNotices] = useState<Record<string, string>>({});
  const [stopping, setStopping] = useState<Record<string, Stopping>>({});
  const [selectedId, setSelectedId] = useState(agents[0].id);
  const [expanded, setExpanded] = useState<{ agentId: string; open: boolean } | null>(null);
  const noticeTimers = useRef(new Map<string, number>());

  const showError = useCallback((agentId: string, message: string) => {
    setErrors((current) => ({ ...current, [agentId]: message }));
  }, []);
  const clearError = useCallback((agentId: string) => {
    setErrors((current) => {
      if (!(agentId in current)) return current;
      const next = { ...current };
      delete next[agentId];
      return next;
    });
  }, []);

  const registry = useTerminals(projectId, (agentId) => isLive(sessions.byAgent[agentId]), showError);
  const watch = useHooksWatch(projectId);
  const avatarOf = useAgentAvatars(projectId, agents.map((agent) => agent.id));
  const tintOf = useAvatarTints(
    agents.map((agent) => agent.id),
    avatarOf,
  );
  const choices = useStartChoices(projectId);
  const preflight = useHooksPreflight(projectId);

  // The selected card's terminal is the one being read: make sure the session has its size.
  useEffect(() => {
    registry?.scheduleResize(selectedId, true);
  }, [registry, selectedId]);

  // Moving into or out of the dialog changes the size; send it even if a debounce merged it.
  useEffect(() => {
    if (expanded) registry?.scheduleResize(expanded.agentId, true);
  }, [registry, expanded]);

  useEffect(() => {
    const timers = noticeTimers.current;
    return () => timers.forEach((timer) => window.clearTimeout(timer));
  }, []);

  function notify(agentId: string, message: string) {
    window.clearTimeout(noticeTimers.current.get(agentId));
    setNotices((current) => ({ ...current, [agentId]: message }));
    noticeTimers.current.set(
      agentId,
      window.setTimeout(() => {
        noticeTimers.current.delete(agentId);
        setNotices((current) => {
          const next = { ...current };
          delete next[agentId];
          return next;
        });
      }, NOTICE_MS),
    );
  }

  async function start(agentId: string, choice: StartChoice, installedHooks: boolean): Promise<AppError | null> {
    clearError(agentId);
    // A fresh screen for the new session; its first output can arrive before agentStart answers.
    registry?.reset(agentId);
    const outcome = await sessions.start(
      agentId,
      choice.kind === "task" ? { promptFile: choice.file.path, sliceId: choice.file.sliceId } : {},
    );
    if (installedHooks) notify(agentId, "Installed Foundry's hooks before starting.");
    if (!outcome.ok) {
      // Nothing started: put the last session's output back.
      registry?.refill(agentId);
      return outcome.error;
    }
    // A new session starts at 120x30, whatever this terminal's size.
    registry?.scheduleResize(agentId, true);
    watch.arm(agentId, outcome.data.since);
    return null;
  }

  async function stop(agentId: string, force: boolean) {
    clearError(agentId);
    setStopping((current) => ({ ...current, [agentId]: force ? "force" : "graceful" }));
    const outcome = await sessions.stop(agentId, force);
    setStopping((current) => {
      // A force stop sent during a graceful one owns the state now.
      if (current[agentId] !== (force ? "force" : "graceful")) return current;
      const next = { ...current };
      delete next[agentId];
      return next;
    });
    if (!outcome.ok) showError(agentId, outcome.error.message);
  }

  const expandedAgent = expanded ? (agents.find((agent) => agent.id === expanded.agentId) ?? null) : null;
  const selected = agents.some((agent) => agent.id === selectedId) ? selectedId : agents[0].id;

  return (
    <div className="flex flex-col gap-3">
      {sessions.error && (
        <ErrorLine message={sessions.error.message}>
          <PressButton variant="ghost" size="xs" onClick={sessions.reload} className="-my-0.5 shrink-0">
            Try again
          </PressButton>
        </ErrorLine>
      )}
      <div className="grid grid-cols-[repeat(auto-fit,minmax(420px,1fr))] gap-2">
        {agents.map((agent) => (
          <AgentCard
            key={agent.id}
            projectId={projectId}
            agent={agent}
            avatar={avatarOf(agent.id)}
            tint={tintOf(agent.id)}
            session={sessions.byAgent[agent.id]}
            registry={registry}
            selected={agent.id === selected}
            onSelect={() => setSelectedId(agent.id)}
            expanded={expanded?.open === true && expanded.agentId === agent.id}
            onExpand={() => setExpanded({ agentId: agent.id, open: true })}
            choices={choices}
            preflight={preflight}
            onStart={(choice, installedHooks) => start(agent.id, choice, installedHooks)}
            stopping={stopping[agent.id]}
            onStop={(force) => void stop(agent.id, force)}
            hooksSilent={watch.silent.has(agent.id)}
            notice={notices[agent.id] ?? null}
            error={errors[agent.id] ?? null}
            onDismissError={() => clearError(agent.id)}
          />
        ))}
      </div>
      <ExpandedTerminal
        registry={registry}
        agent={expandedAgent}
        avatar={expandedAgent ? avatarOf(expandedAgent.id) : ""}
        tint={expandedAgent ? tintOf(expandedAgent.id) : undefined}
        session={expandedAgent ? sessions.byAgent[expandedAgent.id] : undefined}
        open={expanded?.open === true}
        onClose={() => setExpanded((current) => (current ? { ...current, open: false } : null))}
      />
    </div>
  );
}
