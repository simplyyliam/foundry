import type { ReactNode } from "react";
import type { RoadmapResult } from "@/lib/bindings";
import { Artifact, Bullets } from "./Artifact";

function Group({ heading, items }: { heading: string; items: readonly ReactNode[] }) {
  return (
    <div className="flex flex-col gap-1">
      <p className="text-[12px] font-medium text-muted-foreground">{heading}</p>
      <Bullets items={items} />
    </div>
  );
}

/**
 * Everything the roadmap couldn't account for: requirements with no matching brain-dump
 * excerpt, requirements placed in no slice, and brain-dump paragraphs nothing picked up.
 * Renders nothing when all three are empty.
 */
export function NeedsALook({ result }: { result: RoadmapResult }) {
  const unsourced = [...result.roadmap.requirements, ...result.unassigned]
    .filter((requirement) => requirement.unsourced)
    .map((requirement) => requirement.text);
  const unassigned = result.unassigned.map((requirement) => requirement.text);
  const uncovered = result.uncoveredParagraphs;

  if (unsourced.length + unassigned.length + uncovered.length === 0) return null;

  return (
    <Artifact title="Needs a look">
      <div className="flex flex-col gap-2.5">
        {unsourced.length > 0 && <Group heading="Not found in the brain dump" items={unsourced} />}
        {unassigned.length > 0 && <Group heading="Not placed in a slice" items={unassigned} />}
        {uncovered.length > 0 && (
          <Group
            heading="Brain dump the roadmap didn't pick up"
            items={uncovered.map((paragraph, index) => (
              <span key={index} className="line-clamp-3" title={paragraph}>
                {paragraph}
              </span>
            ))}
          />
        )}
      </div>
    </Artifact>
  );
}
