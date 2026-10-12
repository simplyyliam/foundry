/**
 * A fake for `src/lib/bindings.ts` in tests. `src/test/setup.ts` swaps the module's `commands`
 * and `events` for the ones here, so components run unchanged against typed, scripted answers.
 *
 *   backend.respond("getPlan", plan);                         // every call answers ok(plan)
 *   backend.respond("getPlan", (projectId) => planFor(projectId));
 *   backend.fail("commitSetup", { code: "git_identity_missing", message: "…" });
 *   backend.respondOnce("setSliceStatus", plan);              // next call only, before `respond`
 *   backend.failOnce("setSliceStatus", { code: "setup_not_committed", message: "…" });
 *   backend.throwOnce("getPlan", new Error("IPC down"));      // a real IPC failure throws
 *   backend.calls("setSliceStatus");                          // [[projectId, sliceId, …], …]
 *   backend.emit("planChanged", { projectId, revision, id, ts });
 *
 * Names, arguments, answers and event payloads are all typed from the real bindings. A
 * command with nothing registered rejects with an error naming it (components show that as
 * an `internal` error) and is listed in `backend.unhandled`, so a missing stub is easy to spot.
 */
import type { AppError, commands as realCommands, events as realEvents } from "@/lib/bindings";

type Commands = typeof realCommands;
type Events = typeof realEvents;

export type CommandName = keyof Commands;
export type CommandArgs<K extends CommandName> = Parameters<Commands[K]>;
type Result<K extends CommandName> = Awaited<ReturnType<Commands[K]>>;
/** What the command's `{ status: "ok" }` result carries. */
export type CommandData<K extends CommandName> = Extract<Result<K>, { status: "ok" }>["data"];

export type EventName = keyof Events;
/** The payload a listener of `events[K]` receives. */
export type EventPayload<K extends EventName> = Parameters<Parameters<Events[K]["listen"]>[0]>[0]["payload"];

/** Builds an error answer for a function handler: `respond("x", () => error(code, message))`. */
export function error(code: AppError["code"], message: string): FakeError {
  return { [ERROR]: { code, message } };
}
const ERROR = Symbol("fake-backend-error");
type FakeError = { [ERROR]: AppError };

type Answer<K extends CommandName> = CommandData<K> | FakeError;
type Handler<K extends CommandName> = (...args: CommandArgs<K>) => Answer<K> | Promise<Answer<K>>;
/** One registered reply: a fixed answer, a function of the arguments, or a throw. */
type Reply<K extends CommandName> =
  | { kind: "value"; value: Answer<K> }
  | { kind: "handler"; handler: Handler<K> }
  | { kind: "throw"; error: unknown };

export interface Call {
  command: CommandName;
  args: unknown[];
}

type AnyCallback = (event: { event: string; id: number; payload: unknown }) => void;

const isFakeError = (value: unknown): value is FakeError =>
  typeof value === "object" && value !== null && ERROR in value;

const asReply = <K extends CommandName>(answer: Answer<K> | Handler<K>): Reply<K> =>
  typeof answer === "function" ? { kind: "handler", handler: answer as Handler<K> } : { kind: "value", value: answer };

function createBackend() {
  const always = new Map<CommandName, Reply<CommandName>>();
  const queued = new Map<CommandName, Reply<CommandName>[]>();
  const log: Call[] = [];
  const unhandled: CommandName[] = [];
  const listeners = new Map<string, Set<AnyCallback>>();
  let eventId = 0;

  async function invoke(command: CommandName, args: unknown[]) {
    log.push({ command, args });
    const reply = queued.get(command)?.shift() ?? always.get(command);
    if (!reply) {
      unhandled.push(command);
      throw new Error(`fake backend: no response registered for "${command}"`);
    }
    if (reply.kind === "throw") throw reply.error;
    const answer =
      reply.kind === "value" ? reply.value : await (reply.handler as (...a: unknown[]) => unknown)(...args);
    return isFakeError(answer) ? { status: "error", error: answer[ERROR] } : { status: "ok", data: answer };
  }

  const commands = new Proxy({} as Commands, {
    get: (_target, name) => (...args: unknown[]) => invoke(name as CommandName, args),
  });

  function fakeEvent(name: string) {
    const listen = (callback: AnyCallback) => {
      const set = listeners.get(name) ?? new Set();
      listeners.set(name, set);
      set.add(callback);
      return Promise.resolve(() => void set.delete(callback));
    };
    const once = (callback: AnyCallback) => {
      const wrapped: AnyCallback = (event) => {
        listeners.get(name)?.delete(wrapped);
        callback(event);
      };
      return listen(wrapped);
    };
    const emit = (payload: unknown) => {
      deliver(name, payload);
      return Promise.resolve();
    };
    const base = { listen, once, emit };
    // `events.x(window).listen(…)` works too, like the real binding.
    return Object.assign(() => base, base);
  }

  function deliver(name: string, payload: unknown) {
    const event = { event: name, id: ++eventId, payload };
    [...(listeners.get(name) ?? [])].forEach((callback) => callback(event));
  }

  // The Tauri event name of each binding event, as `bindings.ts` registers it.
  const EVENT_NAMES: Record<EventName, string> = {
    agentEvent: "agent:event",
    agentOutput: "agent:output",
    agentStatus: "agent:status",
    appReady: "app:ready",
    planChanged: "plan:changed",
    roadmapProgress: "roadmap:progress",
    verifyProgress: "verify:progress",
    verifySkipCleared: "verify:skip_cleared",
    verifySkipped: "verify:skipped",
  };
  const events = Object.fromEntries(
    Object.entries(EVENT_NAMES).map(([key, name]) => [key, fakeEvent(name)]),
  ) as unknown as Events;

  return {
    /** The fake `commands` and `events`, installed in place of the real ones by `setup.ts`. */
    commands,
    events,

    /** Every call answers with `answer` (or what the function returns for the call's arguments). */
    respond<K extends CommandName>(command: K, answer: CommandData<K> | Handler<K>) {
      always.set(command, asReply<K>(answer) as Reply<CommandName>);
    },
    /** Every call answers `{ status: "error", error }`. */
    fail<K extends CommandName>(command: K, appError: AppError) {
      always.set(command, { kind: "value", value: { [ERROR]: appError } });
    },
    /** The next call answers this; queued answers are used in order, before `respond`/`fail`. */
    respondOnce<K extends CommandName>(command: K, answer: CommandData<K> | Handler<K>) {
      enqueue(command, asReply<K>(answer));
    },
    failOnce<K extends CommandName>(command: K, appError: AppError) {
      enqueue(command, { kind: "value", value: { [ERROR]: appError } });
    },
    /** The next call throws (rejects) instead of answering, like a failed IPC call. */
    throwOnce(command: CommandName, thrown: unknown) {
      enqueue(command, { kind: "throw", error: thrown });
    },

    /** The arguments of every call to `command`, oldest first. */
    calls<K extends CommandName>(command: K): CommandArgs<K>[] {
      return log.filter((call) => call.command === command).map((call) => call.args as CommandArgs<K>);
    },
    /** Every call to any command, in order. */
    get log(): readonly Call[] {
      return log;
    },
    /** Commands that were called with nothing registered. */
    get unhandled(): readonly CommandName[] {
      return unhandled;
    },

    /** Delivers an event to every current listener of `events[name]`, synchronously. */
    emit<K extends EventName>(name: K, payload: EventPayload<K>) {
      deliver(EVENT_NAMES[name], payload);
    },
    /** How many listeners `events[name]` has right now. */
    listenerCount(name: EventName): number {
      return listeners.get(EVENT_NAMES[name])?.size ?? 0;
    },

    /** Forgets every answer, call and listener. `setup.ts` runs it after each test. */
    reset() {
      always.clear();
      queued.clear();
      log.length = 0;
      unhandled.length = 0;
      listeners.clear();
      eventId = 0;
    },
  };

  function enqueue<K extends CommandName>(command: K, reply: Reply<K>) {
    const list = queued.get(command) ?? [];
    list.push(reply as Reply<CommandName>);
    queued.set(command, list);
  }
}

export const backend = createBackend();
export type FakeBackend = typeof backend;
