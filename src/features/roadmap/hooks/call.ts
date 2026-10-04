import type { AppError } from "@/lib/bindings";

export type Outcome<T> = { ok: true; data: T } | { ok: false; error: AppError };

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

/** Turns a binding call (which can also throw, e.g. outside Tauri) into an Outcome. */
export async function call<T>(request: () => Promise<Result<T>>): Promise<Outcome<T>> {
  try {
    const result = await request();
    return result.status === "ok" ? { ok: true, data: result.data } : { ok: false, error: result.error };
  } catch (error) {
    // Not an AppError (IPC itself failed): pass its own message through.
    return {
      ok: false,
      error: { code: "internal", message: error instanceof Error ? error.message : String(error) },
    };
  }
}
