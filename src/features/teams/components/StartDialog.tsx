import { CircleCheck, Info, ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { AppError, TeamAgent } from "@/lib/bindings";
import { PressButton } from "@/shared";
import type { HooksCheck, HooksPreflight } from "../hooks/useHooksPreflight";
import { ErrorLine } from "./ErrorLine";
import type { StartChoice } from "./StartMenu";

interface StartDialogProps {
  agent: TeamAgent;
  /** The pick from the Start menu; the dialog is open while it's set. */
  choice: StartChoice | null;
  onClose: () => void;
  preflight: HooksPreflight;
  /** Starts the session. Resolves to the backend's refusal, or null once it's started. */
  onConfirm: (choice: StartChoice, installedHooks: boolean) => Promise<AppError | null>;
}

/** "This starts a real Claude Code session…", with the hooks check first. */
export function StartDialog({ agent, choice, onClose, preflight, onConfirm }: StartDialogProps) {
  // Kept through the close animation, so the text doesn't blank out while it fades.
  const [shown, setShown] = useState(choice);
  // Each pick gets a fresh body (its own hooks check and state).
  const [round, setRound] = useState(0);
  if (choice && choice !== shown) {
    setShown(choice);
    setRound(round + 1);
  }

  return (
    <Dialog open={choice !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        {shown && <StartBody key={round} agent={agent} choice={shown} preflight={preflight} onConfirm={onConfirm} />}
      </DialogContent>
    </Dialog>
  );
}

type Check = { status: "checking" } | ({ status: "done" } & HooksCheck);

function StartBody({
  agent,
  choice,
  preflight,
  onConfirm,
}: {
  agent: TeamAgent;
  choice: StartChoice;
  preflight: HooksPreflight;
  onConfirm: StartDialogProps["onConfirm"];
}) {
  const [check, setCheck] = useState<Check>({ status: "checking" });
  const [phase, setPhase] = useState<"idle" | "installing" | "starting">("idle");
  const [error, setError] = useState<string | null>(null);
  const [autoInstallFailed, setAutoInstallFailed] = useState(false);
  const [manualInstall, setManualInstall] = useState<{ busy: boolean; error: string | null; done: boolean }>({
    busy: false,
    error: null,
    done: false,
  });

  useEffect(() => {
    let cancelled = false;
    void preflight.check().then((result) => {
      if (!cancelled) setCheck({ status: "done", ...result });
    });
    return () => {
      cancelled = true;
    };
  }, [preflight]);

  const done = check.status === "done" ? check : null;
  const missing = done?.hooks.ok === true && !done.hooks.data.installed;
  const autoInstall = done?.settings.ok === true && done.settings.data.autoInstallHooks;
  const willAutoInstall = missing && autoInstall && !autoInstallFailed;
  const busy = phase !== "idle";

  const adoptStatus = (hooks: HooksCheck["hooks"]) =>
    setCheck((current) => (current.status === "done" ? { ...current, hooks } : current));

  async function installByHand() {
    setManualInstall({ busy: true, error: null, done: false });
    const outcome = await preflight.install();
    if (outcome.ok) {
      adoptStatus({ ok: true, data: outcome.data.status });
      setManualInstall({ busy: false, error: null, done: true });
    } else setManualInstall({ busy: false, error: outcome.error.message, done: false });
  }

  async function start() {
    setError(null);
    let installed = false;
    if (willAutoInstall) {
      setPhase("installing");
      const outcome = await preflight.install();
      if (!outcome.ok) {
        setError(outcome.error.message);
        setAutoInstallFailed(true);
        setPhase("idle");
        return;
      }
      adoptStatus({ ok: true, data: outcome.data.status });
      installed = true;
    }
    setPhase("starting");
    const refusal = await onConfirm(choice, installed);
    // On success the dialog closes; this component unmounts with it.
    if (refusal) {
      setError(refusal.message);
      setPhase("idle");
    }
  }

  const startLabel =
    phase === "installing"
      ? "Installing hooks…"
      : phase === "starting"
        ? "Starting…"
        : autoInstallFailed && missing
          ? "Start without hooks"
          : "Start";

  return (
    <>
      <DialogHeader>
        <DialogTitle>Start {agent.displayName}?</DialogTitle>
        <DialogDescription>This starts a real Claude Code session and uses your Claude usage.</DialogDescription>
      </DialogHeader>

      <div className="flex flex-col gap-3">
        <div className="rounded-lg bg-muted px-3 py-2.5 text-[13px]">
          {choice.kind === "none" ? (
            <>
              <p className="font-medium">Without a task</p>
              <p className="text-muted-foreground">It reads its role, then waits for you to type.</p>
            </>
          ) : (
            <>
              <p className="font-medium">{choice.sliceName}</p>
              <p className="truncate font-mono text-[11.5px] text-muted-foreground" title={choice.file.path}>
                {choice.file.path}
              </p>
            </>
          )}
        </div>

        {check.status === "checking" && <p className="text-[12.5px] text-muted-foreground">Checking hooks…</p>}
        {done && !done.hooks.ok && <ErrorLine message={done.hooks.error.message} />}
        {done && !done.settings.ok && <ErrorLine message={done.settings.error.message} />}

        {willAutoInstall && (
          <p className="flex items-start gap-2 text-[12.5px] text-muted-foreground">
            <Info aria-hidden className="mt-0.5 size-3.5 shrink-0" />
            Hooks aren't installed in this project. Auto-install hooks is on, so Foundry installs them first.
          </p>
        )}

        {/* Also when getSettings failed: we can't tell whether to auto-install, so offer it by hand. */}
        {missing && !autoInstall && (
          <div role="alert" className="flex flex-col gap-2 rounded-lg bg-destructive/8 px-3 py-2.5">
            <p className="flex items-start gap-2 text-[12.5px]">
              <ShieldAlert aria-hidden className="mt-0.5 size-3.5 shrink-0 text-destructive" />
              Write limits aren't enforced until hooks are installed
            </p>
            <PressButton
              size="sm"
              variant="outline"
              className="self-start"
              disabled={manualInstall.busy || busy}
              onClick={() => void installByHand()}
            >
              {manualInstall.busy ? "Installing…" : "Install hooks"}
            </PressButton>
            {manualInstall.error && <p className="text-[12.5px]">{manualInstall.error}</p>}
          </div>
        )}

        {manualInstall.done && (
          <p className="flex items-center gap-2 text-[12.5px] text-muted-foreground">
            <CircleCheck aria-hidden className="size-3.5 shrink-0 text-status-running" />
            Hooks installed.
          </p>
        )}

        {error && <ErrorLine message={error} />}
      </div>

      <DialogFooter>
        <DialogClose render={<PressButton variant="outline" disabled={busy} />}>Cancel</DialogClose>
        <PressButton disabled={check.status === "checking" || busy} onClick={() => void start()}>
          {startLabel}
        </PressButton>
      </DialogFooter>
    </>
  );
}
