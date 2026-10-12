import { GitCommitHorizontal, LoaderCircle, TriangleAlert, X } from "lucide-react";
import type { ReactNode } from "react";
import type { CommitSetupResult } from "@/lib/bindings";
import { PressButton, SetupSection } from "@/shared";
import { useSetupCommit } from "../hooks/useSetupCommit";

const plural = (count: number, one: string) => `${count} ${count === 1 ? one : `${one}s`}`;

/**
 * "Commit setup", shown only while `setupStatus` says `needsCommit`, listing the files it will
 * commit. After a commit, the files committed and the short id stay until dismissed. Nothing
 * renders while there's nothing to commit and nothing to report.
 */
export function CommitSetupSection({ projectId }: { projectId: string }) {
  const { load, reload, committing, error, result, commit, dismissResult } = useSetupCommit(projectId);
  const files = load.status === "ready" && load.value.needsCommit ? load.value.files : null;

  if (load.status === "loading" || (load.status === "ready" && !files && !result && !error)) return null;

  return (
    <SetupSection
      title="Commit setup"
      intro="Each slice starts from a commit, so Foundry's setup files need to be committed first. Only these files are committed."
    >
      {load.status === "error" && (
        <ErrorLine message={load.error.message}>
          <PressButton size="xs" variant="outline" className="rounded-md" onClick={reload}>
            Try again
          </PressButton>
        </ErrorLine>
      )}

      {result && <Committed result={result} onDismiss={dismissResult} />}

      {files && (
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <p className="text-[12.5px] text-muted-foreground">{plural(files.length, "file")} to commit</p>
            <ul
              aria-label="Files to commit"
              className="flex max-h-56 flex-col gap-0.5 overflow-y-auto rounded-lg bg-muted/50 px-3 py-2 font-mono text-[12px]"
            >
              {files.map((file) => (
                <li key={file} className="truncate" title={file}>
                  {file}
                </li>
              ))}
            </ul>
          </div>
          <PressButton className="w-fit" disabled={committing} onClick={() => void commit()}>
            {committing ? (
              <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" />
            ) : (
              <GitCommitHorizontal aria-hidden data-icon="inline-start" />
            )}
            {committing ? "Committing…" : "Commit setup"}
          </PressButton>
        </div>
      )}

      {error && <ErrorLine message={error.message} />}
    </SetupSection>
  );
}

/** What the commit committed: the files and the short id. */
function Committed({ result, onDismiss }: { result: CommitSetupResult; onDismiss: () => void }) {
  const short = result.commit.slice(0, 7);
  return (
    <div role="status" className="flex items-start gap-2 rounded-xl bg-muted/60 py-2 pr-1.5 pl-3 text-[13px]">
      <GitCommitHorizontal aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
      <div className="flex min-w-0 flex-1 flex-col gap-1 py-px">
        {result.files.length === 0 ? (
          <p>Nothing to commit: the setup files were already committed.</p>
        ) : (
          <>
            <p>
              Committed {plural(result.files.length, "setup file")}
              {short && (
                <>
                  {" "}
                  as <code className="font-mono text-[12px] select-text">{short}</code>
                </>
              )}
            </p>
            <ul aria-label="Committed files" className="flex flex-col gap-0.5 font-mono text-[12px] text-muted-foreground">
              {result.files.map((file) => (
                <li key={file} className="truncate" title={file}>
                  {file}
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
      <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={onDismiss} className="rounded-full">
        <X aria-hidden />
      </PressButton>
    </div>
  );
}

/** The backend's message as-is, selectable (git's fix is a command to copy). */
function ErrorLine({ message, children }: { message: string; children?: ReactNode }) {
  return (
    <div role="alert" className="flex items-start gap-2 rounded-lg bg-destructive/8 py-2 pr-1.5 pl-3 text-[12.5px]">
      <TriangleAlert aria-hidden className="mt-px size-3.5 shrink-0 text-destructive" />
      <p className="min-w-0 flex-1 leading-snug break-words whitespace-pre-wrap select-text">{message}</p>
      {children}
    </div>
  );
}
