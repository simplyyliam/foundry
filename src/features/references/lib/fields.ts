import type { BakeryPatch } from "@/lib/bindings";

export type FieldKey = keyof BakeryPatch;

/** Every editable field of a reference, with its wording. */
export const FIELDS: Record<FieldKey, { label: string; placeholder: string; list?: boolean; hint?: string }> = {
  label: { label: "Label", placeholder: "Label" },
  take: { label: "Take", placeholder: "What do you like about this?", hint: "What to copy" },
  skip: {
    label: "Skip",
    placeholder: "What should agents ignore?",
    hint: "What to ignore: the more specific, the less the agent guesses",
  },
  tags: { label: "Tags", placeholder: "Tags, comma separated", list: true },
  appliesTo: { label: "Applies to", placeholder: "Agents or tasks, comma separated", list: true },
};

/** Asked before a Skip is saved empty (clearing it): agents then read the image as all-in. */
export const EMPTY_SKIP_CONFIRM = "Skip is empty, so agents may copy everything. Save anyway?";
