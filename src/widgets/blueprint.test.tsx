import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, commitStatus, plan, slice, summary } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Blueprint } from "./blueprint";

const REFUSAL =
  "Commit the setup files before starting a slice: AGENTS.md, .gitignore are new or edited. Commit them, then start again.";

describe("Blueprint slice sheet", () => {
  it("saves workspace access on flip, and Commit setup after a refused start retries the start", async () => {
    const foundation = slice({ id: "s-foundation", name: "Foundation" });
    backend.respond("getPlan", plan({ revision: 1, slices: [foundation] }));
    backend.respond("planSummary", summary());
    backend.respond("commitStatus", commitStatus());
    backend.respond("agentList", []);

    const { user } = renderWithApp(
      <Blueprint projectId={PROJECT_ID} requirementsHref={`/projects/${PROJECT_ID}/requirements`} />,
    );

    // Opened from the keyboard: React Flow sets `pointer-events: none` on node wrappers and the
    // box's `pointer-events-auto` class has no effect without the stylesheet in jsdom.
    const box = await screen.findByRole("button", { name: /^Foundation: .*Open details$/ });
    act(() => box.focus());
    await user.keyboard("{Enter}");
    const sheet = await screen.findByRole("dialog");

    // Workspace access: flipping saves through the plan queue with the revision we hold.
    backend.respondOnce("setSliceWorkspaceAccess", plan({ revision: 2, slices: [{ ...foundation, workspaceAccess: true }] }));
    const access = within(sheet).getByRole("switch", { name: "Allow root files and lockfile" });
    expect(access).not.toBeChecked();
    await user.click(access);
    await waitFor(() => expect(access).toBeChecked());
    expect(backend.calls("setSliceWorkspaceAccess")).toEqual([[PROJECT_ID, "s-foundation", true, 1]]);

    // Starting is refused until setup is committed: the message shows as-is, with the fix.
    backend.failOnce("setSliceStatus", { code: "setup_not_committed", message: REFUSAL });
    backend.respondOnce(
      "setSliceStatus",
      plan({ revision: 3, slices: [{ ...foundation, workspaceAccess: true, status: "in_progress", ready: false }] }),
    );
    backend.respondOnce("commitSetup", { commit: "abc1234def5678", files: [".gitignore", "AGENTS.md"] });

    const status = within(sheet).getByRole("group", { name: "Status" });
    await user.click(within(status).getByRole("button", { name: "in progress" }));
    expect(await within(sheet).findByText(REFUSAL)).toBeInTheDocument();

    await user.click(within(sheet).getByRole("button", { name: "Commit setup" }));

    // The commit is reported, then the same move is retried and lands.
    const committed = await within(sheet).findByText(/Committed 2 setup files/);
    expect(committed).toHaveTextContent("as abc1234");
    await waitFor(() => expect(within(status).getByRole("button", { name: "in progress" })).toHaveAttribute("aria-pressed", "true"));
    expect(within(sheet).queryByText(REFUSAL)).not.toBeInTheDocument();
    expect(within(sheet).getByText("Takes effect when an agent next starts on this slice")).toBeInTheDocument();

    expect(backend.calls("commitSetup")).toEqual([[PROJECT_ID]]);
    expect(backend.calls("setSliceStatus")).toEqual([
      [PROJECT_ID, "s-foundation", "in_progress", { override: false, reason: null }, 2],
      [PROJECT_ID, "s-foundation", "in_progress", { override: false, reason: null }, 2],
    ]);
    expect(backend.unhandled).toEqual([]);
  });
});
