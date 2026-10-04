import type { Roadmap } from "@/lib/bindings";
import { buildOrder, countBySlice } from "../lib/order";
import { Artifact, BodyText, Bullets, MutedLine, SectionHeading } from "./Artifact";

/** Narrow pane: one column. Wide (the pop-out): two. */
const GRID = "grid gap-3 @2xl:grid-cols-2";

export function GoalSection({ goal }: { goal: string }) {
  if (!goal.trim()) return null;
  return (
    <section className="flex flex-col gap-1">
      <SectionHeading>Goal</SectionHeading>
      <BodyText>{goal}</BodyText>
    </section>
  );
}

/** Each user's name and description as plain text, then their pain points in a box. */
export function TargetUsersSection({ users }: { users: Roadmap["targetUsers"] }) {
  if (users.length === 0) return null;
  return (
    <section className="flex flex-col gap-2">
      <SectionHeading>Target users</SectionHeading>
      <div className="grid gap-5 @2xl:grid-cols-2">
        {users.map((user) => (
          <div key={user.name} className="flex flex-col gap-1.5">
            <h4 className="text-[13px] leading-snug font-medium text-foreground">{user.name}</h4>
            {user.description && <BodyText>{user.description}</BodyText>}
            {user.painPoints.length > 0 && (
              <Artifact title="Pain points:" className="mt-1.5">
                <Bullets items={user.painPoints} />
              </Artifact>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

/** One box per feature family: description, features, and what it works with. */
export function FeatureMapSection({ families }: { families: Roadmap["featureMap"] }) {
  if (families.length === 0) return null;
  return (
    <section className="flex flex-col gap-2">
      <SectionHeading>Feature map</SectionHeading>
      <div className={GRID}>
        {families.map((family) => (
          <Artifact key={family.family} title={family.family} description={family.description || undefined}>
            {family.features.length > 0 && <Bullets items={family.features} />}
            {family.relatesTo.length > 0 && (
              <div className="mt-1.5">
                <MutedLine>Works with: {family.relatesTo.join(", ")}</MutedLine>
              </div>
            )}
          </Artifact>
        ))}
      </div>
    </section>
  );
}

/** Slices in dependency order, each with what it comes after and how many requirements it has. */
export function BuildOrderSection({ roadmap }: { roadmap: Roadmap }) {
  if (roadmap.slices.length === 0) return null;
  const counts = countBySlice(roadmap.requirements);
  return (
    <section className="flex flex-col gap-2">
      <SectionHeading>Build order</SectionHeading>
      <Artifact>
        <ol className="flex flex-col gap-2">
          {buildOrder(roadmap.slices).map((slice, index) => {
            const count = counts.get(slice.name) ?? 0;
            return (
              <li key={slice.name} className="flex gap-2">
                <span className="w-4 shrink-0 text-right text-[12px] leading-[1.45] text-muted-foreground tabular-nums">
                  {index + 1}.
                </span>
                <div className="min-w-0">
                  <p className="text-[13px] leading-snug text-foreground/85">{slice.name}</p>
                  {slice.dependsOn.length > 0 && <MutedLine>after: {slice.dependsOn.join(", ")}</MutedLine>}
                  <MutedLine>
                    {count} {count === 1 ? "requirement" : "requirements"}
                  </MutedLine>
                </div>
              </li>
            );
          })}
        </ol>
      </Artifact>
    </section>
  );
}

/** Deferred items, each anchored after a slice. */
export function LaterSection({ deferred }: { deferred: Roadmap["deferred"] }) {
  if (deferred.length === 0) return null;
  return (
    <section className="flex flex-col gap-2">
      <SectionHeading>Later</SectionHeading>
      <Artifact>
        <ul className="flex flex-col gap-1.5">
          {deferred.map((item) => (
            <li key={`${item.label}:${item.anchoredTo}`}>
              <p className="text-[13px] leading-snug text-foreground/85">{item.label}</p>
              <MutedLine>after {item.anchoredTo}</MutedLine>
            </li>
          ))}
        </ul>
      </Artifact>
    </section>
  );
}

export function OpenQuestionsSection({ questions }: { questions: readonly string[] }) {
  if (questions.length === 0) return null;
  return (
    <section className="flex flex-col gap-2">
      <SectionHeading>Open questions</SectionHeading>
      <Artifact>
        <Bullets items={questions} />
      </Artifact>
    </section>
  );
}
