import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { RoadmapConflict } from "@/lib/bindings";
import { resetConflicts } from "@/features/roadmap/hooks/useConflicts";
import { SidebarProvider } from "@/shared";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, plan } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { RoadmapPanel } from "./roadmap";

afterEach(resetConflicts);

const minimap: RoadmapConflict = {
  id: "c-1",
  topic: "Minimap",
  brainDumpExcerpt: "No minimap, it clutters the map.",
  otherSource: { kind: "bakery", ref: "03-red-door.png" },
  otherExcerpt: "Keep the corner minimap.",
  suggested: "reference_wins",
  ruling: null,
  ruleRequirementId: null,
};

/** The Roadmap panel with these conflicts at plan revision `revision`. */
async function renderPanel(conflicts: RoadmapConflict[], revision = 6) {
  backend.respond("getRoadmap", null);
  backend.respond("getDocument", "Some brain dump.");
  backend.respond("claudeStatus", {
    path: "claude",
    version: "2.0.0",
    loggedIn: true,
    authMethod: "claude.ai",
    apiProvider: "firstParty",
    subscriptionType: "max",
    problem: null,
  });
  backend.respond("getPlan", plan({ revision, unresolvedConflicts: conflicts.filter((conflict) => !conflict.ruling).length }));
  backend.respond("listRoadmapConflicts", conflicts);
  const rendered = renderWithApp(
    <SidebarProvider>
      <RoadmapPanel projectId={PROJECT_ID} />
    </SidebarProvider>,
  );
  const section = await screen.findByRole("region", { name: "Conflicts to settle" });
  return { ...rendered, section };
}

/** What `resolveRoadmapConflict` / `unresolveRoadmapConflict` answer: the conflict as it now is. */
function resolved(conflict: RoadmapConflict, revision: number) {
  return { conflict, ruleRequirementId: conflict.ruleRequirementId ?? null, plan: plan({ revision }) };
}

describe("Roadmap conflicts", () => {
  it("shows both excerpts with their sources, the unsettled count and the suggested ruling first", async () => {
    const { section } = await renderPanel([minimap, { ...minimap, id: "c-2", topic: "Font" }]);

    expect(within(section).getByLabelText("2 unsettled")).toHaveTextContent("2");
    const card = within(section).getByRole("group", { name: "Minimap" });
    expect(within(card).getByText("No minimap, it clutters the map.")).toBeInTheDocument();
    expect(within(card).getByText("docs/brain-dump.md")).toBeInTheDocument();
    expect(within(card).getByText("Keep the corner minimap.")).toBeInTheDocument();
    expect(within(card).getByText("Bakery")).toBeInTheDocument();
    expect(within(card).getByText("03-red-door.png")).toBeInTheDocument();

    const buttons = within(card).getAllByRole("button").map((button) => button.textContent);
    expect(buttons.slice(0, 3)).toEqual(["Follow the referenceSuggested", "Follow the brain dump", "Write my own"]);
    expect(within(card).getByRole("checkbox", { name: "Add as a project-wide rule" })).toBeChecked();
  });

  it("follows the reference as a project-wide rule, then collapses to one line", async () => {
    const { user, section } = await renderPanel([minimap]);

    backend.respondOnce(
      "resolveRoadmapConflict",
      resolved({ ...minimap, ruling: { kind: "reference_wins", text: null }, ruleRequirementId: "r-rule" }, 7),
    );
    await user.click(within(section).getByRole("button", { name: /Follow the reference/ }));

    await waitFor(() =>
      expect(within(section).getByRole("group", { name: "Minimap" })).toHaveTextContent("Minimap: Follow the reference · rule"),
    );
    const line = within(section).getByRole("group", { name: "Minimap" });
    expect(within(line).getByRole("button", { name: "Undo" })).toBeInTheDocument();
    expect(within(section).getByText("All settled")).toBeInTheDocument();
    expect(backend.calls("resolveRoadmapConflict")).toEqual([
      [PROJECT_ID, "c-1", { kind: "reference_wins", text: null }, true, 6],
    ]);
  });

  it("follows the brain dump without a rule when the box is unticked", async () => {
    const { user, section } = await renderPanel([minimap]);

    await user.click(within(section).getByRole("checkbox", { name: "Add as a project-wide rule" }));
    backend.respondOnce("resolveRoadmapConflict", resolved({ ...minimap, ruling: { kind: "brain_dump_wins", text: null } }, 7));
    await user.click(within(section).getByRole("button", { name: "Follow the brain dump" }));

    await waitFor(() => expect(within(section).getByRole("group", { name: "Minimap" })).toHaveTextContent("Minimap: Follow the brain dump"));
    expect(within(section).getByRole("group", { name: "Minimap" })).not.toHaveTextContent("rule");
    expect(backend.calls("resolveRoadmapConflict")).toEqual([
      [PROJECT_ID, "c-1", { kind: "brain_dump_wins", text: null }, false, 6],
    ]);
  });

  it("writes my own ruling in a text box", async () => {
    const { user, section } = await renderPanel([minimap]);

    await user.click(within(section).getByRole("button", { name: "Write my own" }));
    const save = within(section).getByRole("button", { name: "Save ruling" });
    expect(save).toBeDisabled();
    await user.type(within(section).getByRole("textbox", { name: "Your ruling" }), "  Minimap only on wide screens ");

    const ruling = { kind: "custom" as const, text: "Minimap only on wide screens" };
    backend.respondOnce("resolveRoadmapConflict", resolved({ ...minimap, ruling, ruleRequirementId: "r-rule" }, 7));
    await user.click(save);

    await waitFor(() =>
      expect(within(section).getByRole("group", { name: "Minimap" })).toHaveTextContent("Minimap: “Minimap only on wide screens”"),
    );
    expect(backend.calls("resolveRoadmapConflict")).toEqual([[PROJECT_ID, "c-1", ruling, true, 6]]);
  });

  it("Undo reopens the card through unresolve_roadmap_conflict", async () => {
    const settled = { ...minimap, ruling: { kind: "reference_wins" as const, text: null }, ruleRequirementId: "r-rule" };
    const { user, section } = await renderPanel([settled]);

    backend.respondOnce("unresolveRoadmapConflict", resolved(minimap, 7));
    await user.click(within(section).getByRole("button", { name: "Undo" }));

    const card = await within(section).findByRole("button", { name: /Follow the reference/ });
    expect(card).toBeInTheDocument();
    expect(within(section).getByLabelText("1 unsettled")).toBeInTheDocument();
    expect(backend.calls("unresolveRoadmapConflict")).toEqual([[PROJECT_ID, "c-1", 6]]);

    // The next ruling sends the revision Undo returned.
    backend.respondOnce("resolveRoadmapConflict", resolved({ ...minimap, ruling: { kind: "brain_dump_wins", text: null } }, 8));
    await user.click(within(section).getByRole("button", { name: "Follow the brain dump" }));
    await waitFor(() => expect(backend.calls("resolveRoadmapConflict")).toHaveLength(1));
    expect(backend.calls("resolveRoadmapConflict")[0][4]).toBe(7);
  });

  it("shows a stale revision on the card like other plan edits", async () => {
    const { user, section } = await renderPanel([minimap]);

    backend.failOnce("resolveRoadmapConflict", { code: "stale_revision", message: "The plan is at revision 9, not 6." });
    await user.click(within(section).getByRole("button", { name: /Follow the reference/ }));

    const alert = await within(section).findByRole("alert");
    expect(alert).toHaveTextContent("The plan changed elsewhere, so it was reloaded. Try again.");
    expect(alert).toHaveTextContent("The plan is at revision 9, not 6.");
    // Re-read after the refusal.
    await waitFor(() => expect(backend.calls("listRoadmapConflicts").length).toBeGreaterThan(1));
  });

  it("shows no panel without conflicts", async () => {
    backend.respond("getRoadmap", null);
    backend.respond("getDocument", "Some brain dump.");
    backend.respond("claudeStatus", { path: "claude", version: null, loggedIn: true, authMethod: null, apiProvider: null, subscriptionType: null, problem: null });
    backend.respond("getPlan", plan());
    backend.respond("listRoadmapConflicts", []);
    renderWithApp(
      <SidebarProvider>
        <RoadmapPanel projectId={PROJECT_ID} />
      </SidebarProvider>,
    );
    await waitFor(() => expect(backend.calls("listRoadmapConflicts")).toHaveLength(1));
    expect(await screen.findByText("No roadmap yet.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Conflicts to settle" })).not.toBeInTheDocument();
  });
});
