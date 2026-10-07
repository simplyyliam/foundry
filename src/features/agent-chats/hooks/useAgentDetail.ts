import { useCallback, useEffect, useRef, useState } from "react";
import { commands, events, type AgentEvent, type Severity } from "@/lib/bindings";
import { isAlert, mergeEvents, worse, type DetailTab } from "../lib/events";
import { call } from "./call";
import { readUnreadWorst } from "./unread";

const PAGE = 100;
/** "Load older" keeps paging while a page adds nothing to the tab, up to this many pages. */
const MAX_PAGES = 5;

export interface EventStream {
  /** Newest first. */
  events: AgentEvent[];
  status: "loading" | "ready" | "error";
  /** The first page's error, as-is. */
  error: string | null;
  /** The oldest event has been loaded. */
  exhausted: boolean;
  loadingOlder: boolean;
  /** The last "Load older" error, as-is. */
  olderError: string | null;
}

export interface AgentDetail {
  /** Null until the default tab is known (Danger when there are unread warnings or dangers). */
  tab: DetailTab | null;
  setTab: (tab: DetailTab) => void;
  /** Every event: Implementations and Activity. */
  all: EventStream;
  /** `minSeverity: warn`: the Danger tab. */
  alerts: EventStream;
  /** The worst unread warning/danger the Danger tab hasn't shown yet (its tab dot). */
  pendingAlert: Exclude<Severity, "info"> | null;
  /** The backend's message when the unread state couldn't be read; nothing is marked read then. */
  unreadError: string | null;
  /** The last `agentMarkRead` error, as-is. */
  markError: string | null;
  dismissMarkError: () => void;
  loadOlder: (tab: DetailTab) => void;
  retry: () => void;
}

const LOADING: EventStream = { events: [], status: "loading", error: null, exhausted: false, loadingOlder: false, olderError: null };

const matches = (tab: DetailTab) => (event: AgentEvent) =>
  tab === "danger" ? isAlert(event) : !isAlert(event) && (event.kind === "turn_complete") === (tab === "implementations");

/**
 * One agent's events for the detail view (shared by the panel and its pop-out).
 *
 * Marking read: the backend has one marker per agent, so marking from any tab marks every
 * tab. While the agent has an unread warning or danger, `agentMarkRead` waits until the
 * Danger tab has been shown (with its events loaded); otherwise it runs when the detail opens
 * and as events arrive. Always only while `visible`, and only up to the newest id loaded.
 */
export function useAgentDetail(
  projectId: string,
  agentId: string,
  { visible, onMarked }: { visible: boolean; onMarked: () => void },
): AgentDetail {
  const [tab, setTab] = useState<DetailTab | null>(null);
  const [all, setAll] = useState<EventStream>(LOADING);
  const [alerts, setAlerts] = useState<EventStream>(LOADING);
  const [pendingAlert, setPendingAlert] = useState<AgentDetail["pendingAlert"]>(null);
  /** Whether the unread state is known; nothing is marked read until it is. */
  const [known, setKnown] = useState(false);
  const [unreadError, setUnreadError] = useState<string | null>(null);
  const [markError, setMarkError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  const dangerShown = visible && tab === "danger" && alerts.status === "ready";
  const latest = useRef({ dangerShown, all, alerts, onMarked });
  useEffect(() => {
    latest.current = { dangerShown, all, alerts, onMarked };
  });

  // Listen first, then read, so nothing between the two is missed (merging drops duplicates).
  useEffect(() => {
    let cancelled = false;
    let stop: (() => void) | undefined;
    events.agentEvent
      .listen(({ payload }) => {
        if (payload.projectId !== projectId || payload.agentId !== agentId) return;
        setAll((stream) => ({ ...stream, events: mergeEvents(stream.events, [payload]) }));
        if (!isAlert(payload)) return;
        setAlerts((stream) => ({ ...stream, events: mergeEvents(stream.events, [payload]) }));
        if (!latest.current.dangerShown) {
          setPendingAlert((current) => worse(current, payload.severity) as AgentDetail["pendingAlert"]);
        }
      })
      .then((unlisten) => {
        if (cancelled) unlisten();
        else stop = unlisten;
      })
      .catch((reason) => console.warn("Couldn't listen to agent events:", reason));

    void readUnreadWorst(projectId, agentId).then((outcome) => {
      if (cancelled) return;
      if (!outcome.ok) {
        // Unknown: never mark read in this visit, rather than risk marking an unseen warning.
        setUnreadError(outcome.error.message);
        setTab((current) => current ?? "implementations");
        return;
      }
      const alert = outcome.data === "warn" || outcome.data === "danger" ? outcome.data : null;
      setPendingAlert((current) => worse(current, alert) as AgentDetail["pendingAlert"]);
      setTab((current) => current ?? (alert ? "danger" : "implementations"));
      setKnown(true);
    });

    const first = (minSeverity: Severity | null, set: typeof setAll) =>
      void call(() => commands.agentEvents(projectId, agentId, { beforeId: null, limit: PAGE, minSeverity })).then(
        (outcome) => {
          if (cancelled) return;
          set((stream) =>
            outcome.ok
              ? { ...stream, status: "ready", error: null, events: mergeEvents(stream.events, outcome.data), exhausted: outcome.data.length < PAGE }
              : { ...stream, status: "error", error: outcome.error.message },
          );
        },
      );
    first(null, setAll);
    first("warn", setAlerts);

    return () => {
      cancelled = true;
      stop?.();
    };
  }, [projectId, agentId, attempt]);

  // Marking read.
  const lastMarked = useRef(0);
  const newestId = Math.max(all.events[0]?.id ?? 0, alerts.events[0]?.id ?? 0);
  useEffect(() => {
    if (!known || !visible || all.status !== "ready" || newestId === 0) return;
    if (pendingAlert) {
      // Wait for the Danger tab; once it's on screen, the warnings count as seen.
      if (dangerShown) setPendingAlert(null);
      return;
    }
    if (newestId <= lastMarked.current) return;
    const previous = lastMarked.current;
    lastMarked.current = newestId;
    void call(() => commands.agentMarkRead(projectId, agentId, newestId)).then((outcome) => {
      if (outcome.ok) {
        setMarkError(null);
        latest.current.onMarked();
      } else {
        if (lastMarked.current === newestId) lastMarked.current = previous; // Try again on the next event.
        setMarkError(outcome.error.message);
      }
    });
  }, [projectId, agentId, known, visible, pendingAlert, dangerShown, all.status, newestId]);

  const loadOlder = useCallback(
    (forTab: DetailTab) => {
      const set = forTab === "danger" ? setAlerts : setAll;
      const stream = forTab === "danger" ? latest.current.alerts : latest.current.all;
      if (stream.status !== "ready" || stream.exhausted || stream.loadingOlder) return;
      const minSeverity: Severity | null = forTab === "danger" ? "warn" : null;
      const wanted = matches(forTab);
      set((current) => ({ ...current, loadingOlder: true, olderError: null }));
      void (async () => {
        let beforeId = stream.events[stream.events.length - 1]?.id ?? null;
        for (let page = 0; page < MAX_PAGES; page++) {
          const outcome = await call(() => commands.agentEvents(projectId, agentId, { beforeId, limit: PAGE, minSeverity }));
          if (!outcome.ok) {
            set((current) => ({ ...current, loadingOlder: false, olderError: outcome.error.message }));
            return;
          }
          const exhausted = outcome.data.length < PAGE;
          set((current) => ({ ...current, events: mergeEvents(current.events, outcome.data), exhausted }));
          if (exhausted || outcome.data.some(wanted)) break;
          beforeId = outcome.data[outcome.data.length - 1].id;
        }
        set((current) => ({ ...current, loadingOlder: false }));
      })();
    },
    [projectId, agentId],
  );

  // A tab with nothing in the first page looks further back on its own, once per tab.
  const autoLoaded = useRef(new Set<DetailTab>());
  useEffect(() => {
    if (!tab || autoLoaded.current.has(tab)) return;
    const stream = tab === "danger" ? alerts : all;
    if (stream.status !== "ready" || stream.exhausted || stream.events.some(matches(tab))) return;
    autoLoaded.current.add(tab);
    loadOlder(tab);
  }, [tab, all, alerts, loadOlder]);

  const retry = useCallback(() => {
    setAll(LOADING);
    setAlerts(LOADING);
    setUnreadError(null);
    autoLoaded.current.clear();
    setAttempt((value) => value + 1);
  }, []);

  return {
    tab,
    setTab,
    all,
    alerts,
    pendingAlert,
    unreadError,
    markError,
    dismissMarkError: useCallback(() => setMarkError(null), []),
    loadOlder,
    retry,
  };
}
