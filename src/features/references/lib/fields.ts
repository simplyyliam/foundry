import type { BakeryPatch } from "@/lib/bindings";

export type FieldKey = keyof BakeryPatch;

/** Every editable field of a reference, with its wording. */
export const FIELDS: Record<FieldKey, { label: string; placeholder: string; list?: boolean }> = {
  label: { label: "Label", placeholder: "Label" },
  take: { label: "Take", placeholder: "What do you like about this?" },
  skip: { label: "Skip", placeholder: "What should agents ignore?" },
  tags: { label: "Tags", placeholder: "Tags, comma separated", list: true },
  appliesTo: { label: "Applies to", placeholder: "Agents or tasks, comma separated", list: true },
};
