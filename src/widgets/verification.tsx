import { AutoVerifyNotices, VerificationView } from "@/features/verification";

export interface VerificationProps {
  /** The project from the route. */
  projectId: string;
}

/**
 * The Verification page: test commands, what needs a look, and each slice's report and
 * verification. Keyed by project, so drafts and the open sheet never carry over.
 */
export function Verification({ projectId }: VerificationProps) {
  return <VerificationView key={projectId} projectId={projectId} />;
}

/**
 * App-wide, mounted once in the app layout: listens to `verify:progress` and runs auto mode
 * (`report_ready` → verify), showing what auto mode couldn't do wherever the user is.
 */
export function AutoVerify() {
  return <AutoVerifyNotices />;
}
