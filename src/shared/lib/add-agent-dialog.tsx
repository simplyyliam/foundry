import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import type { TeamWritten } from "@/lib/bindings";

/**
 * Opens the one Add agent dialog (mounted in the app layout) for a project. Lives in
 * `@/shared` so Setup and Agent Chats can both open it without importing each other.
 */
export interface AddAgentRequest {
  projectId: string;
  /** Runs after `addAgentManually` succeeds (every team view also refreshes on its own). */
  onAdded?: (written: TeamWritten) => void;
}

interface AddAgentDialogState {
  /** What the open dialog is for; null when it's closed. */
  request: AddAgentRequest | null;
  openAddAgent: (projectId: string, options?: Omit<AddAgentRequest, "projectId">) => void;
  close: () => void;
}

const AddAgentDialogContext = createContext<AddAgentDialogState | null>(null);

export function AddAgentDialogProvider({ children }: { children: ReactNode }) {
  const [request, setRequest] = useState<AddAgentRequest | null>(null);

  const openAddAgent = useCallback((projectId: string, options?: Omit<AddAgentRequest, "projectId">) => {
    setRequest({ projectId, ...options });
  }, []);
  const close = useCallback(() => setRequest(null), []);

  const value = useMemo(() => ({ request, openAddAgent, close }), [request, openAddAgent, close]);
  return <AddAgentDialogContext.Provider value={value}>{children}</AddAgentDialogContext.Provider>;
}

export function useAddAgentDialog(): AddAgentDialogState {
  const context = useContext(AddAgentDialogContext);
  if (!context) throw new Error("useAddAgentDialog must be used inside AddAgentDialogProvider");
  return context;
}
