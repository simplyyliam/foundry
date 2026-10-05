import { useState } from "react";
import { ChevronRight } from "lucide-react";
import type { BakeryItem } from "@/lib/bindings";
import { PressButton } from "@/shared";
import { updateReference } from "../hooks/useBakery";
import { FIELDS, type FieldKey } from "../lib/fields";
import { parseList } from "../lib/reference";
import { CommitField } from "./CommitField";

interface ReferenceFieldsProps {
  projectId: string;
  item: BakeryItem;
  /** Shown directly, in this order. */
  visible: FieldKey[];
  /** Behind a "More" toggle. */
  more: FieldKey[];
}

/** A card's editable fields, each saved on blur or Enter through `bakeryUpdate`. */
export function ReferenceFields({ projectId, item, visible, more }: ReferenceFieldsProps) {
  const [showMore, setShowMore] = useState(false);
  return (
    <div className="flex flex-col gap-1.5">
      {visible.map((key) => (
        <Field key={key} projectId={projectId} item={item} field={key} />
      ))}
      {more.length > 0 && (
        <>
          <PressButton
            variant="ghost"
            size="xs"
            aria-expanded={showMore}
            onClick={() => setShowMore((open) => !open)}
            className="-ml-1.5 h-6 w-fit px-1.5 text-[11px] text-muted-foreground"
          >
            <ChevronRight aria-hidden data-icon="inline-start" className={showMore ? "rotate-90 transition-transform" : "transition-transform"} />
            More
          </PressButton>
          {showMore && more.map((key) => <Field key={key} projectId={projectId} item={item} field={key} />)}
        </>
      )}
    </div>
  );
}

export function Field({
  projectId,
  item,
  field,
  multiline,
}: {
  projectId: string;
  item: BakeryItem;
  field: FieldKey;
  multiline?: boolean;
}) {
  const spec = FIELDS[field];
  const raw = item[field];
  const value = Array.isArray(raw) ? raw.join(", ") : raw;
  return (
    <CommitField
      value={value}
      label={spec.label}
      placeholder={spec.placeholder}
      multiline={multiline}
      onCommit={(text) => updateReference(projectId, item.id, { [field]: spec.list ? parseList(text) : text })}
    />
  );
}
