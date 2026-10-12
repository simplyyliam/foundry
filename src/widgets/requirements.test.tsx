import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, plan, requirement, slice, summary } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Requirements } from "./requirements";

describe("Requirements widget", () => {
  it("moves a constraint into Project rules with Make project-wide", async () => {
    const feed = slice({ id: "s-feed", name: "Feed" });
    const rule = requirement({ id: "r-1", text: "Every screen works offline", kind: "constraint" });
    backend.respond("getPlan", plan({ revision: 4, slices: [feed], requirements: [rule] }));
    backend.respond("planSummary", summary({ unassigned: ["r-1"], unconfirmed: ["r-1"] }));

    const { user } = renderWithApp(<Requirements projectId={PROJECT_ID} />);

    const unassigned = await screen.findByRole("region", { name: "Unassigned" });
    expect(within(unassigned).getByText(rule.text)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Project rules" })).not.toBeInTheDocument();
    // Not handled yet: it can't be confirmed.
    expect(within(unassigned).getByRole("button", { name: "Confirm" })).toBeDisabled();

    backend.respondOnce(
      "setRequirementProjectWide",
      plan({ revision: 5, slices: [feed], requirements: [{ ...rule, projectWide: true }] }),
    );
    backend.respond("planSummary", summary({ unconfirmed: ["r-1"] }));

    await user.click(within(unassigned).getByRole("button", { name: "More actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Make project-wide" }));

    const rules = await screen.findByRole("region", { name: "Project rules" });
    expect(within(rules).getByText(rule.text)).toBeInTheDocument();
    expect(within(rules).getByText("Apply to every slice")).toBeInTheDocument();
    // Project-wide counts as handled, so it can be confirmed now.
    expect(within(rules).getByRole("button", { name: "Confirm" })).toBeEnabled();
    expect(screen.queryByRole("region", { name: "Unassigned" })).not.toBeInTheDocument();

    expect(backend.calls("setRequirementProjectWide")).toEqual([[PROJECT_ID, "r-1", true, 4]]);

    // And the same menu now offers the way back.
    await user.click(within(rules).getByRole("button", { name: "More actions" }));
    expect(await screen.findByRole("menuitem", { name: "Assign to a slice" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Make project-wide" })).not.toBeInTheDocument();
    expect(backend.unhandled).toEqual([]);
  });
});
