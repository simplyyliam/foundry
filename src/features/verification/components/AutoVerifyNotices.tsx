import { AnimatePresence, motion, useReducedMotionConfig } from "motion/react";
import { ArrowUpRight, TriangleAlert } from "lucide-react";
import { Link } from "react-router-dom";
import { buttonVariants } from "@/components/ui/button";
import { PROJECT_SECTIONS, projectSectionPath, useProject } from "@/features/projects";
import { cn } from "@/lib/utils";
import { usePressScale } from "@/shared";
import { dismissAutoNotice, useAutoNotices, useVerificationListeners, type AutoNotice } from "../hooks/verifyRuns";
import { DismissButton } from "./Notices";

/**
 * App-wide: starts the verification listeners (progress, and auto mode's `report_ready`) and
 * shows what auto mode couldn't do, wherever the user is. Mounted once, in the app layout.
 */
export function AutoVerifyNotices() {
  useVerificationListeners();
  const notices = useAutoNotices();
  const reducedMotion = useReducedMotionConfig();

  return (
    <div
      aria-live="polite"
      className="pointer-events-none absolute right-4 bottom-4 z-40 flex w-[min(24rem,calc(100%-2rem))] flex-col gap-2"
    >
      <AnimatePresence initial={false}>
        {notices.map((notice) => (
          <motion.div
            key={notice.id}
            initial={reducedMotion ? { opacity: 0 } : { opacity: 0, y: 8, filter: "blur(2px)" }}
            animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            exit={reducedMotion ? { opacity: 0 } : { opacity: 0, y: 6 }}
            transition={{ type: "spring", visualDuration: 0.25, bounce: 0.15 }}
            className="pointer-events-auto"
          >
            <NoticeCard notice={notice} />
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}

function NoticeCard({ notice }: { notice: AutoNotice }) {
  const { project } = useProject(notice.projectId);
  const linkRef = usePressScale<HTMLAnchorElement>();
  const section = PROJECT_SECTIONS.find((candidate) => candidate.id === "verification");
  return (
    <div
      role="alert"
      className="flex items-start gap-2.5 rounded-2xl bg-popover py-2.5 pr-1.5 pl-3 text-[12.5px] shadow-lg ring-1 ring-foreground/10"
    >
      <TriangleAlert aria-hidden className="mt-0.5 size-3.5 shrink-0 text-warning" />
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <p className="font-medium">
          {notice.kind === "commands" ? "Auto-verify didn't run" : "Auto-verify failed"}
          {project && <span className="font-normal text-muted-foreground"> · {project.name}</span>}
        </p>
        <p className="leading-snug break-words">{notice.message}</p>
        {section && (
          <Link
            ref={linkRef}
            to={projectSectionPath(notice.projectId, section)}
            onClick={() => dismissAutoNotice(notice.id)}
            className={cn(buttonVariants({ variant: "outline", size: "xs" }), "w-fit rounded-md bg-background")}
          >
            {notice.kind === "commands" ? "Review test commands" : "Open Verification"}
            <ArrowUpRight aria-hidden data-icon="inline-end" />
          </Link>
        )}
      </div>
      <DismissButton onClick={() => dismissAutoNotice(notice.id)} />
    </div>
  );
}
