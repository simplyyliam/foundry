import { CircleCheck, FileText, LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import type { ManagerDigest } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { Outcome } from "../hooks/call";
import { DismissButton, ErrorLine } from "./Notices";

const WRITTEN_DISMISS_MS = 6000;

interface VerificationHeaderProps {
  verified: number;
  total: number;
  writeDigest: () => Promise<Outcome<ManagerDigest>>;
}

/** "N of M slices verified" and Write digest. */
export function VerificationHeader({ verified, total, writeDigest }: VerificationHeaderProps) {
  const { busy, error, run, clear } = useAction();
  const [written, setWritten] = useState(0);

  useEffect(() => {
    if (written === 0) return;
    const timer = setTimeout(() => setWritten(0), WRITTEN_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [written]);

  const onWrite = async () => {
    setWritten(0);
    const outcome = await run(writeDigest);
    if (outcome.ok) setWritten(Date.now());
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <p className="mr-auto text-[13px] text-muted-foreground tabular-nums">
          <span className="font-medium text-foreground">
            {verified} of {total}
          </span>{" "}
          {total === 1 ? "slice" : "slices"} verified
        </p>
        <PressButton size="sm" variant="secondary" disabled={busy} onClick={() => void onWrite()}>
          {busy ? (
            <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" />
          ) : (
            <FileText aria-hidden data-icon="inline-start" />
          )}
          Write digest for Jake
        </PressButton>
      </div>
      {written > 0 && (
        <div role="status" className="flex items-center gap-2 rounded-lg bg-muted/70 py-1.5 pr-1.5 pl-3 text-[12.5px] ring-1 ring-foreground/5">
          <CircleCheck aria-hidden className="size-3.5 shrink-0 text-status-running" />
          <p className="min-w-0 flex-1 leading-snug">
            <code className="font-mono text-[12px]">.foundry/reviews/digest.md</code> written. The Manager agent can read it.
          </p>
          <DismissButton onClick={() => setWritten(0)} />
        </div>
      )}
      {error && (
        <ErrorLine message={error.message}>
          <DismissButton onClick={clear} />
        </ErrorLine>
      )}
    </div>
  );
}
