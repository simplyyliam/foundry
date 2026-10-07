import { useEffect, useRef, useState } from "react";
import { TerminalRegistry } from "./terminalRegistry";

/**
 * The project's terminal registry: created on mount, disposed (every terminal with it) on
 * unmount. Null for the first render, and briefly under StrictMode's double mount.
 */
export function useTerminals(
  projectId: string,
  isLive: (agentId: string) => boolean,
  onError: (agentId: string, message: string) => void,
): TerminalRegistry | null {
  const [registry, setRegistry] = useState<TerminalRegistry | null>(null);
  const latest = useRef({ isLive, onError });
  useEffect(() => {
    latest.current = { isLive, onError };
  });

  useEffect(() => {
    const next = new TerminalRegistry(projectId, {
      isLive: (agentId) => latest.current.isLive(agentId),
      onError: (agentId, message) => latest.current.onError(agentId, message),
    });
    setRegistry(next);
    return () => {
      next.dispose();
      setRegistry((current) => (current === next ? null : current));
    };
  }, [projectId]);

  return registry;
}
