import { useEffect, useState } from "react";
import { commands } from "@/lib/bindings";
import { call } from "./call";

export type BuiltinAvatars = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; ids: string[] };

/** The built-in avatar ids (`builtinAvatarList`), read when the avatar popover opens. */
export function useBuiltinAvatars(): BuiltinAvatars {
  const [state, setState] = useState<BuiltinAvatars>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    void call(() => commands.builtinAvatarList()).then((outcome) => {
      if (cancelled) return;
      setState(outcome.ok ? { status: "ready", ids: outcome.data } : { status: "error", message: outcome.error.message });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return state;
}
