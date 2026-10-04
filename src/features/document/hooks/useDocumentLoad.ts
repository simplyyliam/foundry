import { useEffect, useState } from "react";
import type { AppError } from "@/lib/bindings";
import { loadDocumentFile, type LoadedDocument } from "./documentFile";

export type DocumentLoad =
  | { status: "loading" }
  | { status: "ready"; document: LoadedDocument }
  | { status: "error"; error: AppError };

/** Loads the project's document once (after any pending save of any project has landed). */
export function useDocumentLoad(projectId: string): DocumentLoad {
  const [load, setLoad] = useState<{ projectId: string; result: DocumentLoad } | null>(null);

  useEffect(() => {
    let cancelled = false;
    void loadDocumentFile(projectId).then((outcome) => {
      if (cancelled) return;
      setLoad({
        projectId,
        result: outcome.ok ? { status: "ready", document: outcome.data } : { status: "error", error: outcome.error },
      });
    });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  return load?.projectId === projectId ? load.result : { status: "loading" };
}
