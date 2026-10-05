import { useEffect } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { RotateCw, TriangleAlert, X } from "lucide-react";
import { cn } from "@/lib/utils";
import type { AppError } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { PlanNotice } from "../hooks/usePlan";

const STALE_TEXT = "The plan changed elsewhere, so it was reloaded. Try again.";
const STALE_DISMISS_MS = 8000;

/** Floating notice over the canvas: a reload after `stale_revision`, or a refused action. */
export function NoticeBanner({ notice, onDismiss }: { notice: PlanNotice | null; onDismiss: () => void }) {
  const reducedMotion = useReducedMotion();

  useEffect(() => {
    // A reload notice is informational and goes away; an error stays until dismissed.
    if (notice?.kind !== "stale") return;
    const timer = setTimeout(onDismiss, STALE_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [notice, onDismiss]);

  return (
    <AnimatePresence>
      {notice && (
        <motion.div
          key={notice.id}
          role={notice.kind === "error" ? "alert" : "status"}
          initial={reducedMotion ? { opacity: 0 } : { opacity: 0, y: -8, filter: "blur(2px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={reducedMotion ? { opacity: 0 } : { opacity: 0, y: -6 }}
          transition={{ type: "spring", visualDuration: 0.25, bounce: 0.15 }}
          className="pointer-events-auto flex max-w-lg items-start gap-2.5 rounded-2xl bg-popover py-2 pr-1.5 pl-3 text-sm shadow-lg ring-1 ring-foreground/10"
        >
          <NoticeText kind={notice.kind} message={notice.message} />
          <DismissButton onClick={onDismiss} />
        </motion.div>
      )}
    </AnimatePresence>
  );
}

/** An action's failure shown inline, next to the control that caused it. */
export function ActionError({ error, className }: { error: AppError | null; className?: string }) {
  if (!error) return null;
  return (
    <div
      role="alert"
      className={cn("flex items-start gap-2 rounded-xl bg-destructive/8 px-3 py-2 text-[13px]", className)}
    >
      <NoticeText kind={error.code === "stale_revision" ? "stale" : "error"} message={error.message} />
    </div>
  );
}

function NoticeText({ kind, message }: { kind: PlanNotice["kind"]; message: string }) {
  const Icon = kind === "stale" ? RotateCw : TriangleAlert;
  return (
    <>
      <Icon
        aria-hidden
        className={cn("mt-0.5 size-4 shrink-0", kind === "stale" ? "text-muted-foreground" : "text-destructive")}
      />
      <div className="min-w-0 flex-1 py-px">
        {kind === "stale" ? (
          <>
            <p className="text-foreground">{STALE_TEXT}</p>
            <p className="text-[12px] text-muted-foreground">{message}</p>
          </>
        ) : (
          <p className="text-foreground">{message}</p>
        )}
      </div>
    </>
  );
}

function DismissButton({ onClick }: { onClick: () => void }) {
  return (
    <PressButton pressScale={0.9} variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={onClick} className="rounded-full">
      <X aria-hidden />
    </PressButton>
  );
}

/** The first load failed: the backend's message, as-is. */
export function LoadFailed({ message }: { message: string }) {
  return (
    <div className="flex h-full w-full items-center justify-center bg-background px-8">
      <p role="alert" className="max-w-md text-center text-sm text-muted-foreground">
        {message}
      </p>
    </div>
  );
}
