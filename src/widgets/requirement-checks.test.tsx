import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { PlanRequirement, RequirementCheck } from "@/lib/bindings";
import { resetDismissedChecks } from "@/features/requirements/hooks/useDismissedChecks";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, plan, requirement, slice, summary } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Requirements } from "./requirements";

afterEach(resetDismissedChecks);

const feed = slice({ id: "s-feed", name: "Feed" });
const exists: RequirementCheck = { type: "file_exists", path: "components.json" };
const noRadix: RequirementCheck = { type: "dependency_absent", name: "@radix-ui/*" };
const noImport: RequirementCheck = { type: "text_absent", glob: "src/**/*.tsx", pattern: "@radix-ui/" };

function rule(overrides: Partial<PlanRequirement> = {}) {
  return requirement({ id: "r-1", text: "Use shadcn on Base UI", kind: "constraint", sliceId: "s-feed", ...overrides });
}

async function openEdit(revision: number, requirements: PlanRequirement[]) {
  backend.respond("getPlan", plan({ revision, slices: [feed], requirements }));
  backend.respond("planSummary", summary());
  const rendered = renderWithApp(<Requirements projectId={PROJECT_ID} />);
  const group = await screen.findByRole("region", { name: "Feed" });
  await rendered.user.click(within(group).getByRole("button", { name: "More actions" }));
  await rendered.user.click(await screen.findByRole("menuitem", { name: "Edit…" }));
  const dialog = await screen.findByRole("dialog", { name: "Edit requirement" });
  return { ...rendered, group, dialog, checks: within(dialog).getByRole("region", { name: "Checks" }) };
}

describe("Requirement checks", () => {
  it("shows a check count chip on the row, and none without checks", async () => {
    backend.respond(
      "getPlan",
      plan({
        slices: [feed],
        requirements: [rule({ checks: [exists, noRadix] }), requirement({ id: "r-2", text: "Posts load", sliceId: "s-feed" })],
      }),
    );
    backend.respond("planSummary", summary());
    renderWithApp(<Requirements projectId={PROJECT_ID} />);

    const group = await screen.findByRole("region", { name: "Feed" });
    expect(within(group).getByText("2 checks")).toBeInTheDocument();
    expect(within(group).getAllByText(/^\d checks?$/)).toHaveLength(1);
  });

  it("lists checks as sentences and removes one through set_requirement_checks with the revision", async () => {
    const { user, checks } = await openEdit(7, [rule({ checks: [exists, noRadix, noImport] })]);

    const list = within(checks).getByRole("list", { name: "Checks" });
    expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "File components.json must exist",
      "Dependency @radix-ui/* must not be present",
      "No match for @radix-ui/ in src/**/*.tsx",
    ]);
    expect(within(checks).getByText(/Verify runs these itself/)).toBeInTheDocument();

    backend.respondOnce("setRequirementChecks", plan({ revision: 8, slices: [feed], requirements: [rule({ checks: [exists, noImport] })] }));
    await user.click(within(checks).getByRole("button", { name: "Remove check: Dependency @radix-ui/* must not be present" }));

    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
    expect(backend.calls("setRequirementChecks")).toEqual([[PROJECT_ID, "r-1", [exists, noImport], 7]]);
    expect(backend.unhandled).toEqual([]);
  });

  it("adds a check of the picked type with the fields it needs", async () => {
    const { user, checks } = await openEdit(3, [rule({ checks: [exists] })]);

    await user.click(within(checks).getByRole("button", { name: "Add check" }));
    const form = within(checks).getByRole("group", { name: "New check" });
    // File must exist by default: one Path field.
    expect(within(form).getByLabelText("Path")).toBeInTheDocument();

    await user.click(within(form).getByRole("combobox", { name: "Type" }));
    await user.click(await screen.findByRole("option", { name: "Text must not appear" }));
    expect(within(form).queryByLabelText("Path")).not.toBeInTheDocument();
    const add = within(form).getByRole("button", { name: "Add" });
    await user.type(within(form).getByLabelText("Files"), "src/**/*.tsx");
    // Still needs the pattern.
    expect(add).toBeDisabled();
    await user.type(within(form).getByLabelText("Pattern (regex)"), "@radix-ui/");

    backend.respondOnce("setRequirementChecks", plan({ revision: 4, slices: [feed], requirements: [rule({ checks: [exists, noImport] })] }));
    await user.click(add);

    await waitFor(() => expect(within(checks).queryByRole("group", { name: "New check" })).not.toBeInTheDocument());
    expect(backend.calls("setRequirementChecks")).toEqual([[PROJECT_ID, "r-1", [exists, noImport], 3]]);
    expect(within(checks).getByText("No match for @radix-ui/ in src/**/*.tsx")).toBeInTheDocument();
    // Saving checks never touched the requirement's other fields.
    expect(backend.calls("updateRequirement")).toHaveLength(0);
  });

  it("shows a stale revision like other plan edits and keeps the new check's fields", async () => {
    const { user, checks } = await openEdit(3, [rule()]);
    await user.click(within(checks).getByRole("button", { name: "Add check" }));
    const form = within(checks).getByRole("group", { name: "New check" });
    await user.type(within(form).getByLabelText("Path"), "components.json");

    backend.failOnce("setRequirementChecks", { code: "stale_revision", message: "The plan is at revision 5, not 3." });
    await user.click(within(form).getByRole("button", { name: "Add" }));

    const alert = await within(checks).findByRole("alert");
    expect(alert).toHaveTextContent("The plan changed elsewhere, so it was reloaded. Try again.");
    expect(alert).toHaveTextContent("The plan is at revision 5, not 3.");
    expect(within(form).getByLabelText("Path")).toHaveValue("components.json");
  });

  it("accepts a suggested check by its index, and dismissing only hides it", async () => {
    const { user, checks, dialog } = await openEdit(2, [rule({ suggestedChecks: [noRadix, noImport] })]);

    const suggested = within(checks).getByRole("group", { name: "Suggested checks" });
    await user.click(within(suggested).getByRole("button", { name: "Dismiss: Dependency @radix-ui/* must not be present" }));
    expect(within(suggested).queryByText("Dependency @radix-ui/* must not be present")).not.toBeInTheDocument();

    backend.respondOnce(
      "acceptRequirementCheck",
      plan({ revision: 3, slices: [feed], requirements: [rule({ checks: [noImport], suggestedChecks: [noRadix] })] }),
    );
    // Index 1 in the full list, even though the dismissed one is hidden.
    await user.click(within(suggested).getByRole("button", { name: "Accept: No match for @radix-ui/ in src/**/*.tsx" }));

    await within(checks).findByRole("list", { name: "Checks" });
    expect(backend.calls("acceptRequirementCheck")).toEqual([[PROJECT_ID, "r-1", 1, 2]]);
    // The only suggestion left is the dismissed one, so the group is gone.
    expect(within(checks).queryByRole("group", { name: "Suggested checks" })).not.toBeInTheDocument();
    expect(backend.calls("setRequirementChecks")).toHaveLength(0);

    // Still hidden after closing and reopening the dialog.
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await user.click(within(screen.getByRole("region", { name: "Feed" })).getByRole("button", { name: "More actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit…" }));
    const reopened = await screen.findByRole("dialog", { name: "Edit requirement" });
    expect(within(reopened).queryByText("Dependency @radix-ui/* must not be present")).not.toBeInTheDocument();
    expect(backend.unhandled).toEqual([]);
  });
});
