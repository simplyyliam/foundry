import { CircleCheck, CircleX } from "lucide-react";
import type { CheckResult } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { describeCheck } from "@/shared";

/**
 * The requirement checks Verify ran, under the requirement each belongs to (in the review's
 * order). Pass or fail is an icon plus a word; a failure adds the backend's detail line.
 */
export function CheckResults({
  results,
  requirementText,
}: {
  results: readonly CheckResult[];
  requirementText: (id: string) => string | undefined;
}) {
  if (results.length === 0) return null;
  const groups = new Map<string, CheckResult[]>();
  for (const result of results) groups.set(result.requirementId, [...(groups.get(result.requirementId) ?? []), result]);
  const failing = results.filter((result) => !result.passed).length;

  return (
    <section aria-labelledby="requirement-checks" className="flex flex-col gap-2.5">
      <h3 id="requirement-checks" className="text-[13.5px] font-semibold">
        Requirement checks
      </h3>
      <p className="-mt-1 text-[12px] text-muted-foreground tabular-nums">
        {failing > 0 ? `${failing} of ${results.length} failed` : results.length === 1 ? "1 passed" : `All ${results.length} passed`}
      </p>
      <ul className="flex flex-col gap-1.5">
        {[...groups].map(([requirementId, checks]) => {
          const text = requirementText(requirementId);
          return (
            <li key={requirementId} className="flex flex-col gap-1.5 rounded-lg px-3 py-2 ring-1 ring-foreground/6">
              <p id={`check-req-${requirementId}`} className="text-[13px] leading-snug">
                <code className="font-mono text-[11.5px] text-muted-foreground">{requirementId}</code>
                {text && <span className="ml-1.5">{text}</span>}
              </p>
              <ul aria-labelledby={`check-req-${requirementId}`} className="flex flex-col gap-1">
                {checks.map((result, index) => (
                  <li key={index}>
                    <CheckResultLine result={result} />
                  </li>
                ))}
              </ul>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function CheckResultLine({ result }: { result: CheckResult }) {
  const Icon = result.passed ? CircleCheck : CircleX;
  return (
    <div className="flex items-start gap-2 text-[12.5px] leading-snug">
      <span
        className={cn(
          "flex w-15 shrink-0 items-center gap-1 font-medium",
          result.passed ? "text-foreground [&>svg]:text-status-running" : "text-destructive",
        )}
      >
        <Icon aria-hidden className="size-3.5 shrink-0" />
        {result.passed ? "Passed" : "Failed"}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="break-words">{describeCheck(result.check)}</span>
        {!result.passed && result.detail.trim() !== "" && (
          <span className="font-mono text-[11.5px] break-words text-muted-foreground select-text">{result.detail}</span>
        )}
      </div>
    </div>
  );
}
