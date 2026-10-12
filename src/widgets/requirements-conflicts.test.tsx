import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { RoadmapConflict } from "@/lib/bindings";
import { resetConflicts } from "@/features/roadmap/hooks/useConflicts";
import { SidebarProvider, resetPanelRequests } from "@/shared";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, plan, requirement, slice, summary } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Requirements } from "./requirements";
import { RoadmapPanel } from "./roadmap";

afterEach(() => {
  resetConflicts();
  resetPanelRequests();
});

const feed = slice({ id: "s-feed", name: "Feed" });
const story = requirement({ id: "r-1", text: "Posts load newest first", sliceId: "s-feed" });
const conflict = (id: string, topic: string): RoadmapConflict => ({
  id,
  topic,
  brainDumpExcerpt: "No minimap.",
  otherSource: { kind: "design_family", ref: "dashboard.md" },
  otherExcerpt: "A minimap in the corner.",
  suggested: "reference_wins",
  ruling: null,
  ruleRequirementId: null,
});

/** The Requirements page with the Roadmap panel beside it, as on the requirements route. */
async function renderPage(unresolvedConflicts: number) {
  const conflicts = [conflict("c-1", "Minimap"), conflict("c-2", "Font")].slice(0, unresolvedConflicts);
  backend.respond("getPlan", plan({ revision: 3, slices: [feed], requirements: [story], unresolvedConflicts }));
  backend.respond("planSummary", summary({ unconfirmed: ["r-1"] }));
  backend.respond("listRoadmapConflicts", conflicts);
  backend.respond("getRoadmap", null);
  backend.respond("getDocument", "Brain dump.");
  backend.respond("claudeStatus", { path: "claude", version: null, loggedIn: true, authMethod: null, apiProvider: null, subscriptionType: null, problem: null });
  const rendered = renderWithApp(
    <SidebarProvider>
      <Requirements projectId={PROJECT_ID} />
      <RoadmapPanel projectId={PROJECT_ID} />
    </SidebarProvider>,
  );
  const confirmAll = await screen.findByRole("button", { name: /Confirm all/ });
  return { ...rendered, confirmAll };
}

describe("Confirming with unsettled conflicts", () => {
  it("asks first, and Confirm anyway confirms", async () => {
    const { user, confirmAll } = await renderPage(2);

    await user.click(confirmAll);
    const dialog = await screen.findByRole("dialog", { name: "2 conflicts are unsettled." });
    expect(dialog).toHaveAccessibleDescription("Agents will pick on their own.");
    expect(backend.calls("confirmAll")).toHaveLength(0);

    backend.respondOnce("confirmAll", plan({ revision: 4, slices: [feed], requirements: [{ ...story, state: "confirmed" }], unresolvedConflicts: 2 }));
    await user.click(within(dialog).getByRole("button", { name: "Confirm anyway" }));

    await waitFor(() => expect(backend.calls("confirmAll")).toEqual([[PROJECT_ID, 3]]));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /unsettled/ })).not.toBeInTheDocument());
  });

  it("Settle them confirms nothing and opens the Roadmap's conflicts", async () => {
    const { user, confirmAll } = await renderPage(1);

    await user.click(confirmAll);
    const dialog = await screen.findByRole("dialog", { name: "1 conflict is unsettled." });
    await user.click(within(dialog).getByRole("button", { name: "Settle them" }));

    // The Roadmap pop-out, with the conflicts in it.
    const popOut = await screen.findByRole("dialog", { name: "Roadmap" });
    const section = await within(popOut).findByRole("region", { name: "Conflicts to settle" });
    expect(within(section).getByRole("group", { name: "Minimap" })).toBeInTheDocument();
    expect(backend.calls("confirmAll")).toHaveLength(0);
  });

  it("confirms straight away without unsettled conflicts", async () => {
    const { user, confirmAll } = await renderPage(0);

    backend.respondOnce("confirmAll", plan({ revision: 4, slices: [feed], requirements: [{ ...story, state: "confirmed" }] }));
    await user.click(confirmAll);

    await waitFor(() => expect(backend.calls("confirmAll")).toEqual([[PROJECT_ID, 3]]));
    expect(screen.queryByRole("dialog", { name: /unsettled/ })).not.toBeInTheDocument();
  });
});
