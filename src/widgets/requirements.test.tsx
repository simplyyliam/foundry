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

  it("locks the kind of a project-wide rule in the edit dialog, and not of a slice requirement", async () => {
    const feed = slice({ id: "s-feed", name: "Feed" });
    const rule = requirement({ id: "r-1", text: "Every screen works offline", kind: "constraint", projectWide: true });
    const story = requirement({ id: "r-2", text: "Posts load newest first", sliceId: "s-feed" });
    backend.respond("getPlan", plan({ revision: 2, slices: [feed], requirements: [rule, story] }));
    backend.respond("planSummary", summary());

    const { user } = renderWithApp(<Requirements projectId={PROJECT_ID} />);

    const rules = await screen.findByRole("region", { name: "Project rules" });
    await user.click(within(rules).getByRole("button", { name: "More actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit…" }));

    let dialog = await screen.findByRole("dialog");
    let kinds = within(dialog).getByRole("radiogroup", { name: "Kind" });
    for (const radio of within(kinds).getAllByRole("radio")) expect(radio).toBeDisabled();
    expect(within(kinds).getByRole("radio", { name: "constraint" })).toHaveAttribute("aria-checked", "true");
    expect(kinds).toHaveAccessibleDescription(
      "Project-wide rules are always constraints. Turn off Project-wide first to change the kind.",
    );
    // The rest of the dialog still edits.
    expect(within(dialog).getByLabelText("Requirement")).toBeEnabled();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    const feedGroup = screen.getByRole("region", { name: "Feed" });
    await user.click(within(feedGroup).getByRole("button", { name: "More actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit…" }));
    dialog = await screen.findByRole("dialog", { name: "Edit requirement" });
    kinds = within(dialog).getByRole("radiogroup", { name: "Kind" });
    for (const radio of within(kinds).getAllByRole("radio")) expect(radio).toBeEnabled();
    expect(within(dialog).queryByText(/Project-wide rules are always constraints/)).not.toBeInTheDocument();

    expect(backend.calls("updateRequirement")).toHaveLength(0);
    expect(backend.unhandled).toEqual([]);
  });
});
