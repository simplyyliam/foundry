import { CircleCheck, CircleDashed, LoaderCircle, Plus, ShieldCheck, X } from "lucide-react";
import { useEffect, useRef, useState, type Ref } from "react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import type { AppError, TestCommands } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { useAction } from "../hooks/useAction";
import type { useTestCommands } from "../hooks/useTestCommands";
import { ErrorLine } from "./Notices";

type Controller = ReturnType<typeof useTestCommands>;

interface Row {
  key: number;
  text: string;
}

let nextKey = 0;
const toRows = (commands: readonly string[]): Row[] => commands.map((text) => ({ key: ++nextKey, text }));
/** What Save sends: the rows as typed, without blank ones. */
const cleaned = (rows: readonly Row[]) => rows.map((row) => row.text).filter((text) => text.trim() !== "");
const sameList = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((item, i) => item === b[i]);

interface TestCommandsCardProps {
  controller: Controller;
  /** For "Review test commands" elsewhere on the page: scrolled to and focused. */
  ref?: Ref<HTMLElement>;
}

/** The commands verification runs, editable, with Save and Confirm. */
export function TestCommandsCard({ controller, ref }: TestCommandsCardProps) {
  const { load } = controller;
  return (
    <section
      ref={ref}
      tabIndex={-1}
      aria-labelledby="test-commands-title"
      className="flex scroll-mt-6 flex-col gap-4 rounded-xl bg-card p-5 ring-1 ring-foreground/8 outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      <header className="flex flex-col gap-1">
        <div className="flex items-center justify-between gap-3">
          <h2 id="test-commands-title" className="text-[14px] font-semibold">
            Test commands
          </h2>
          {load.status === "ready" && <CommandsStatus value={load.value} />}
        </div>
        <p className="text-[13px] leading-snug text-pretty text-muted-foreground">
          Verification runs these commands on your computer, in the project folder. Agents can't change this list
          without you confirming it again.
        </p>
      </header>
      {load.status === "loading" && (
        <div className="flex justify-center py-4">
          <LoaderCircle aria-label="Loading the test commands" className="size-4 animate-spin text-muted-foreground" />
        </div>
      )}
      {load.status === "error" && (
        <ErrorLine message={load.error.message}>
          <PressButton size="xs" variant="outline" className="rounded-md bg-background" onClick={() => void controller.reload()}>
            Try again
          </PressButton>
        </ErrorLine>
      )}
      {load.status === "ready" && <CommandsEditor value={load.value} controller={controller} />}
    </section>
  );
}

function CommandsStatus({ value }: { value: TestCommands }) {
  if (value.commands.length === 0) {
    return (
      <Badge variant="outline" className="h-5 px-2 text-[11px] text-muted-foreground">
        <CircleDashed aria-hidden data-icon="inline-start" />
        None
      </Badge>
    );
  }
  return value.confirmed ? (
    <Badge variant="outline" className="h-5 border-status-running/40 px-2 text-[11px] [&>svg]:text-status-running">
      <CircleCheck aria-hidden data-icon="inline-start" />
      Confirmed
    </Badge>
  ) : (
    <Badge variant="outline" className="h-5 border-warning/50 px-2 text-[11px] [&>svg]:text-warning">
      <CircleDashed aria-hidden data-icon="inline-start" />
      Needs confirmation
    </Badge>
  );
}

function CommandsEditor({ value, controller }: { value: TestCommands; controller: Controller }) {
  const [rows, setRows] = useState<Row[]>(() => toRows(value.commands));
  /** A row to focus after the next render (a new row, or the neighbour of a removed one). */
  const focusKey = useRef<number | null>(null);
  const inputs = useRef(new Map<number, HTMLInputElement>());
  const save = useAction();
  const confirm = useAction();
  /** `test_commands_changed`: its message stays until the next action. */
  const [changed, setChanged] = useState<AppError | null>(null);

  const saved = value.commands;
  const dirty = !sameList(cleaned(rows), saved);

  // A fresh list from disk (after save, confirm or a reload, including `test_commands_changed`)
  // replaces the rows unless they were being edited against the previous list.
  const [lastSaved, setLastSaved] = useState(saved);
  if (!sameList(lastSaved, saved)) {
    setLastSaved(saved);
    if (sameList(cleaned(rows), lastSaved)) setRows(toRows(saved));
  }

  useEffect(() => {
    if (focusKey.current === null) return;
    inputs.current.get(focusKey.current)?.focus();
    focusKey.current = null;
  });

  const addRow = (after?: number) => {
    const row = { key: ++nextKey, text: "" };
    setRows((previous) => {
      const index = after === undefined ? previous.length : previous.findIndex((candidate) => candidate.key === after) + 1;
      return [...previous.slice(0, index), row, ...previous.slice(index)];
    });
    focusKey.current = row.key;
  };

  const removeRow = (key: number) => {
    setRows((previous) => {
      const index = previous.findIndex((row) => row.key === key);
      const next = previous.filter((row) => row.key !== key);
      // Keep keyboard focus in the list: the row above, else the one that took its place.
      const neighbour = next[Math.max(0, index - 1)];
      if (neighbour) focusKey.current = neighbour.key;
      return next;
    });
  };

  const onSave = async () => {
    setChanged(null);
    confirm.clear();
    const list = cleaned(rows);
    const outcome = await save.run(() => controller.save(list));
    if (outcome.ok) setRows(toRows(outcome.data.commands));
  };

  const onConfirm = async () => {
    setChanged(null);
    save.clear();
    // Exactly the list on screen, which is the saved one (Confirm is off while editing).
    const outcome = await confirm.run(() => controller.confirm(saved));
    if (!outcome.ok && outcome.error.code === "test_commands_changed") {
      setChanged(outcome.error);
      confirm.clear();
    }
  };

  const busy = save.busy || confirm.busy;

  return (
    <div className="flex flex-col gap-3">
      {rows.length === 0 ? (
        <p className="rounded-lg border border-dashed border-border px-3 py-3 text-[12.5px] leading-snug text-muted-foreground">
          No commands. Verification will warn that there's nothing to run.
        </p>
      ) : (
        <ul className="flex flex-col gap-1.5" aria-label="Commands">
          {rows.map((row, index) => (
            <li key={row.key} className="flex items-center gap-1.5">
              <Input
                ref={(element) => {
                  if (element) inputs.current.set(row.key, element);
                  else inputs.current.delete(row.key);
                }}
                aria-label={`Command ${index + 1}`}
                value={row.text}
                spellCheck={false}
                autoComplete="off"
                disabled={busy}
                placeholder="e.g. npm test"
                onChange={(event) => {
                  const text = event.target.value;
                  setRows((previous) => previous.map((candidate) => (candidate.key === row.key ? { ...candidate, text } : candidate)));
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
                    event.preventDefault();
                    if (dirty) void onSave();
                  } else if (event.key === "Enter") {
                    event.preventDefault();
                    addRow(row.key);
                  } else if (event.key === "Backspace" && row.text === "" && rows.length > 1) {
                    event.preventDefault();
                    removeRow(row.key);
                  }
                }}
                className="h-8 font-mono text-[12.5px]"
              />
              <PressButton
                type="button"
                variant="ghost"
                size="icon-sm"
                pressScale={0.9}
                aria-label={`Remove command ${index + 1}`}
                disabled={busy}
                onClick={() => removeRow(row.key)}
                className="shrink-0 text-muted-foreground"
              >
                <X aria-hidden />
              </PressButton>
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-wrap items-center gap-2">
        <PressButton type="button" size="sm" variant="ghost" disabled={busy} onClick={() => addRow()}>
          <Plus aria-hidden data-icon="inline-start" />
          Add command
        </PressButton>
        <span className="flex-1" />
        {dirty && (
          <PressButton type="button" size="sm" variant="ghost" disabled={busy} onClick={() => setRows(toRows(saved))}>
            Revert
          </PressButton>
        )}
        <PressButton type="button" size="sm" variant={dirty ? "default" : "secondary"} disabled={!dirty || busy} onClick={() => void onSave()}>
          {save.busy && <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" />}
          Save
        </PressButton>
        {saved.length > 0 && !value.confirmed && (
          <PressButton
            type="button"
            size="sm"
            disabled={dirty || busy}
            title={dirty ? "Save first: Confirm confirms the saved list" : undefined}
            onClick={() => void onConfirm()}
          >
            {confirm.busy ? (
              <LoaderCircle aria-hidden data-icon="inline-start" className="animate-spin" />
            ) : (
              <ShieldCheck aria-hidden data-icon="inline-start" />
            )}
            Confirm
          </PressButton>
        )}
      </div>
      {dirty && (
        <p className="text-[12px] text-muted-foreground">Unsaved changes. Saving makes the list need confirming again.</p>
      )}
      {save.error && <ErrorLine message={save.error.message} />}
      {confirm.error && <ErrorLine message={confirm.error.message} />}
      {changed && (
        <ErrorLine message={changed.message}>
          <span className="self-center pr-1.5 text-[12px] whitespace-nowrap text-muted-foreground">Reloaded below</span>
        </ErrorLine>
      )}
    </div>
  );
}
