import { useEffect, useRef } from "react";

/**
 * Asks the contextual panel to show something from a page that can't reach it (features
 * don't import each other): the Roadmap's conflicts, or one Bakery entry's editor. The
 * layout frame switches the panel to the right view (`usePanelRequests`); that view then
 * takes the request (`usePanelRequest`), even if it only mounts after the switch. The last
 * request waits until taken; a newer one replaces it. Only reaches views in this window.
 */
export type PanelRequest =
  | { kind: "settle-conflicts"; projectId: string }
  | { kind: "bakery-entry"; projectId: string; entryId: string };

type Listener = (request: PanelRequest) => void;

let pending: PanelRequest | null = null;
const listeners = new Set<Listener>();

export function requestPanel(request: PanelRequest) {
  pending = request;
  [...listeners].forEach((listener) => listener(request));
}

/**
 * The waiting request if it's for this view, removed so it's handled once. A plain function
 * on purpose: inside a hook the React Compiler treated `pending` as unchanged across the
 * reassignment and passed the handler null.
 */
function takePending(kind: PanelRequest["kind"], projectId: string): PanelRequest | null {
  const request = pending;
  if (!request || request.kind !== kind || request.projectId !== projectId) return null;
  pending = null;
  return request;
}

/** Every request as it's made, without taking it (the frame, to pick the view). */
export function usePanelRequests(onRequest: Listener) {
  const latest = useRef(onRequest);
  useEffect(() => {
    latest.current = onRequest;
  });
  useEffect(() => {
    const listener: Listener = (request) => latest.current(request);
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);
}

/**
 * Takes requests of `kind` for `projectId`: one made while mounted, or one still waiting when
 * this mounts. A taken request is gone, so a second view of the same kind doesn't repeat it.
 */
export function usePanelRequest<K extends PanelRequest["kind"]>(
  kind: K,
  projectId: string,
  onRequest: (request: Extract<PanelRequest, { kind: K }>) => void,
) {
  const latest = useRef(onRequest);
  useEffect(() => {
    latest.current = onRequest;
  });
  useEffect(() => {
    const take = () => {
      const request = takePending(kind, projectId);
      if (request) latest.current(request as Extract<PanelRequest, { kind: K }>);
    };
    take();
    listeners.add(take);
    return () => {
      listeners.delete(take);
    };
  }, [kind, projectId]);
}

/** Tests only: drop a request nobody took. */
export function resetPanelRequests() {
  pending = null;
}
