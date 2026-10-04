import { useState } from "react";
import {
  DocumentEditor,
  DocumentHeader,
  LoadError,
  SaveStatus,
  useDocumentEditor,
  useDocumentLoad,
  useUncoveredParagraphs,
  type LoadedDocument,
} from "@/features/document";

export interface DocumentProps {
  /** The project whose document this is (from the route). */
  projectId: string;
}

/**
 * A project's markdown document (`docs/brain-dump.md`). Keyed by project, so switching
 * project unmounts the old editor, which saves its pending edit before the new one loads.
 */
export function Document({ projectId }: DocumentProps) {
  return <ProjectDocument key={projectId} projectId={projectId} />;
}

function ProjectDocument({ projectId }: DocumentProps) {
  const load = useDocumentLoad(projectId);
  if (load.status === "loading") return <div className="h-full w-full bg-background" />;
  return (
    <LoadedDocumentView
      projectId={projectId}
      loaded={load.status === "ready" ? load.document : null}
      loadError={load.status === "error" ? load.error.message : null}
    />
  );
}

interface LoadedDocumentViewProps extends DocumentProps {
  loaded: LoadedDocument | null;
  loadError: string | null;
}

function LoadedDocumentView({ projectId, loaded, loadError }: LoadedDocumentViewProps) {
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
  const uncovered = useUncoveredParagraphs(projectId);
  const { editor, status } = useDocumentEditor({ projectId, loaded, uncovered: uncovered.paragraphs });

  return (
    <div className="flex h-full w-full flex-col bg-background">
      <DocumentHeader note={uncovered.error?.message}>
        <SaveStatus status={status} />
      </DocumentHeader>
      <div ref={setScroller} className="min-h-0 flex-1 overflow-y-auto">
        {loadError !== null && <LoadError message={loadError} />}
        <DocumentEditor editor={editor} scrollTarget={scroller} />
      </div>
    </div>
  );
}
