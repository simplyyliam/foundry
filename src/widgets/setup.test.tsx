import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { SetupCommit } from "./setup";

const IDENTITY_MISSING =
  'git doesn\'t know who you are (user.name and user.email not set). Set it in a terminal, then try again: git config --global user.name "Your Name" and git config --global user.email "you@example.com"';

describe("Setup commit section", () => {
  it("shows nothing while setup_status says nothing needs committing", async () => {
    backend.respond("setupStatus", { needsCommit: false, files: [] });

    renderWithApp(<SetupCommit projectId={PROJECT_ID} />);

    await waitFor(() => expect(backend.calls("setupStatus")).toHaveLength(1));
    expect(screen.queryByRole("button", { name: "Commit setup" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Commit setup" })).not.toBeInTheDocument();
    expect(backend.unhandled).toEqual([]);
  });

  it("lists the files it will commit, then shows what it committed and the short id", async () => {
    backend.respondOnce("setupStatus", { needsCommit: true, files: ["AGENTS.md", "agents/roles.json"] });
    backend.respond("setupStatus", { needsCommit: false, files: [] });
    backend.respond("commitSetup", { commit: "abc1234def5678", files: ["AGENTS.md", "agents/roles.json"] });

    const { user } = renderWithApp(<SetupCommit projectId={PROJECT_ID} />);

    const toCommit = await screen.findByRole("list", { name: "Files to commit" });
    expect(within(toCommit).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "AGENTS.md",
      "agents/roles.json",
    ]);
    expect(screen.getByText("2 files to commit")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Commit setup" }));

    const status = await screen.findByRole("status");
    expect(status).toHaveTextContent("Committed 2 setup files as abc1234");
    expect(within(status).getByRole("list", { name: "Committed files" })).toHaveTextContent("AGENTS.mdagents/roles.json");
    // Read again after the commit: nothing left, so the button goes and the result stays.
    await waitFor(() => expect(screen.queryByRole("button", { name: "Commit setup" })).not.toBeInTheDocument());
    expect(screen.getByRole("status")).toBeInTheDocument();

    await user.click(within(status).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.queryByRole("heading", { name: "Commit setup" })).not.toBeInTheDocument());

    expect(backend.calls("commitSetup")).toEqual([[PROJECT_ID]]);
    expect(backend.calls("setupStatus")).toHaveLength(2);
    expect(backend.unhandled).toEqual([]);
  });

  it("shows a failed commit's message as-is, including git's identity instructions", async () => {
    backend.respond("setupStatus", { needsCommit: true, files: ["AGENTS.md"] });
    backend.failOnce("commitSetup", { code: "git_identity_missing", message: IDENTITY_MISSING });

    const { user } = renderWithApp(<SetupCommit projectId={PROJECT_ID} />);

    await user.click(await screen.findByRole("button", { name: "Commit setup" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(IDENTITY_MISSING);
    // Still uncommitted: the button stays for a retry once git is set up.
    expect(screen.getByRole("button", { name: "Commit setup" })).toBeEnabled();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(backend.unhandled).toEqual([]);
  });

  it("reads the status again when the plan changes", async () => {
    backend.respondOnce("setupStatus", { needsCommit: false, files: [] });
    backend.respond("setupStatus", { needsCommit: true, files: [".foundry/plan.json"] });

    renderWithApp(<SetupCommit projectId={PROJECT_ID} />);
    await waitFor(() => expect(backend.calls("setupStatus")).toHaveLength(1));
    expect(screen.queryByRole("button", { name: "Commit setup" })).not.toBeInTheDocument();

    backend.emit("planChanged", { projectId: PROJECT_ID, revision: 2, id: "e-1", ts: 0 });

    expect(await screen.findByRole("button", { name: "Commit setup" })).toBeInTheDocument();
    expect(screen.getByText(".foundry/plan.json")).toBeInTheDocument();
    expect(backend.unhandled).toEqual([]);
  });
});
