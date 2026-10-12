import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AppError, SliceView } from "@/lib/bindings";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, changedFile, commitStatus, digest, plan, slice, sliceDigest, summary, testCommands } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Blueprint } from "./blueprint";
import { Verification } from "./verification";

const MESSAGE = [
  "feat(feed): show the feed list",
  "",
  "Slice: Feed (s-feed)",
  "Agents: builder",
  "Verification: stale",
].join("\n");
const BODY = MESSAGE.split("\n").slice(2).join("\n");

/** Four changes, two of them Foundry's own, and what `commit_slice` would make of them. */
const READY = commitStatus({
  files: [
    changedFile("src/feed.test.ts", { change: "new" }),
    changedFile("src/feed.ts"),
    changedFile(".foundry/reports/s-feed/builder.md", { change: "new", foundryOwned: true }),
    changedFile("AGENTS.md", { foundryOwned: true }),
  ],
  commitType: "feat",
  typeReason: "a new feature file was added",
  message: MESSAGE,
});

const feed = (overrides: Partial<SliceView> = {}) => slice({ id: "s-feed", name: "Feed", status: "in_progress", ...overrides });

async function openBlueprintSheet(sliceView: SliceView = feed()) {
  backend.respond("getPlan", plan({ revision: 1, slices: [sliceView] }));
  backend.respond("planSummary", summary());
  const rendered = renderWithApp(<Blueprint projectId={PROJECT_ID} requirementsHref={`/projects/${PROJECT_ID}/requirements`} />);
  // From the keyboard: jsdom loads no CSS, so the box's `pointer-events-auto` doesn't apply.
  const box = await screen.findByRole("button", { name: /^Feed: .*Open details$/ });
  act(() => box.focus());
  await rendered.user.keyboard("{Enter}");
  const sheet = await screen.findByRole("dialog");
  return { ...rendered, sheet };
}

async function openCommitDialog() {
  backend.respond("commitStatus", READY);
  backend.respond("agentList", []);
  const opened = await openBlueprintSheet();
  const button = await within(opened.sheet).findByRole("button", { name: "Commit changes" });
  await waitFor(() => expect(button).toBeEnabled());
  await opened.user.click(button);
  const dialog = await screen.findByRole("dialog", { name: "Commit “Feed”" });
  await within(dialog).findByRole("textbox", { name: "First line" });
  return { ...opened, dialog };
}

describe("Commit changes on a slice", () => {
  it("is disabled with the reason when there's nothing to commit or a session is live, enabled otherwise", async () => {
    backend.respond("commitStatus", commitStatus());
    backend.respond("agentList", []);
    const { sheet, user } = await openBlueprintSheet();

    // Nothing to commit: disabled, and the reason is the tooltip.
    const nothing = await within(sheet).findByLabelText("Commit changes: Nothing to commit");
    expect(within(nothing).getByRole("button", { name: "Commit changes" })).toBeDisabled();
    await user.hover(nothing);
    expect(await screen.findByText("Nothing to commit")).toBeInTheDocument();
    await user.unhover(nothing);

    // Files to commit, but an agent is running: the session wins.
    backend.respond("commitStatus", READY);
    backend.respond("agentList", [{ agentId: "builder", projectId: PROJECT_ID, status: "running", pid: 42, exitCode: null, since: 10 }]);
    act(() => window.dispatchEvent(new Event("focus")));
    const running = await within(sheet).findByLabelText("Commit changes: A session is still running");
    expect(within(running).getByRole("button", { name: "Commit changes" })).toBeDisabled();

    // The session exits (a newer `agent:status`): the button turns on.
    backend.respond("agentList", []);
    act(() => {
      backend.emit("agentStatus", {
        id: "e1",
        ts: 20,
        projectId: PROJECT_ID,
        agentId: "builder",
        status: "exited",
        exitCode: 0,
        pid: null,
        since: 20,
      });
    });
    await waitFor(() => expect(within(sheet).getByRole("button", { name: "Commit changes" })).toBeEnabled());
    expect(backend.calls("commitStatus").every(([projectId, sliceId]) => projectId === PROJECT_ID && sliceId === "s-feed")).toBe(true);
    expect(backend.calls("commitSlice")).toHaveLength(0);
    expect(backend.unhandled).toEqual([]);
  });

  it("lists the files (Foundry's muted apart), the generated message as given and the type with its reason; untouched, it commits with null", async () => {
    const { dialog, user } = await openCommitDialog();

    const work = within(dialog).getByRole("list", { name: "Changes" });
    expect(within(work).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["newsrc/feed.test.ts", "modifiedsrc/feed.ts"]);
    const foundry = within(dialog).getByRole("list", { name: "Foundry files" });
    expect(within(foundry).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "new.foundry/reports/s-feed/builder.md",
      "modifiedAGENTS.md",
    ]);
    expect(within(dialog).getByText(/4 files to commit/)).toBeInTheDocument();

    // The message exactly as the backend gave it, "Verification: stale" included.
    expect(within(dialog).getByRole("textbox", { name: "First line" })).toHaveValue("feat(feed): show the feed list");
    expect(within(dialog).getByRole("textbox", { name: "Body" })).toHaveValue(BODY);
    expect(within(dialog).getByLabelText("Commit type")).toHaveTextContent("Typefeata new feature file was added");

    backend.respondOnce("commitSlice", {
      commit: "abc1234def5678",
      files: ["AGENTS.md", ".foundry/reports/s-feed/builder.md", "src/feed.test.ts", "src/feed.ts"],
      message: MESSAGE,
      commitType: "feat",
      typeReason: "a new feature file was added",
    });
    backend.respond("commitStatus", commitStatus());
    await user.click(within(dialog).getByRole("button", { name: "Commit" }));

    // Untouched: the backend builds the message itself.
    expect(backend.calls("commitSlice")).toEqual([[PROJECT_ID, "s-feed", null]]);
    expect(await within(dialog).findByRole("status")).toHaveTextContent("Committed 4 files as abc1234");
    // Then it closes by itself.
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Commit “Feed”" })).not.toBeInTheDocument(), { timeout: 4000 });
  });

  it("passes an edited message through as written, and the type follows the first line", async () => {
    const { dialog, user } = await openCommitDialog();
    const subject = within(dialog).getByRole("textbox", { name: "First line" });
    await user.clear(subject);
    await user.type(subject, "test(feed): cover the empty state");
    expect(within(dialog).getByLabelText("Commit type")).toHaveTextContent("Typetestfrom your first line");

    const body = within(dialog).getByRole("textbox", { name: "Body" });
    await user.type(body, "\nReviewed by hand.");

    backend.respondOnce("commitSlice", { commit: "fedcba987", files: ["src/feed.ts"], message: "x", commitType: null, typeReason: null });
    await user.click(within(dialog).getByRole("button", { name: "Commit" }));
    expect(backend.calls("commitSlice")).toEqual([
      [PROJECT_ID, "s-feed", `test(feed): cover the empty state\n\n${BODY}\nReviewed by hand.`],
    ]);
    expect(await within(dialog).findByRole("status")).toHaveTextContent("Committed 1 file as fedcba9");
  });

  it("shows each refusal as-is and stays open", async () => {
    const { dialog, user } = await openCommitDialog();
    const refusals: AppError[] = [
      {
        code: "git_identity_missing",
        message:
          "git doesn't know who you are. Set your name and email, then commit again:\n  git config --global user.name \"Your Name\"\n  git config --global user.email you@example.com",
      },
      { code: "session_running", message: "stop this project's agent sessions before committing" },
      { code: "commit_failed", message: "error: unable to index file 'big.bin'" },
    ];
    for (const refusal of refusals) {
      backend.failOnce("commitSlice", refusal);
      await user.click(within(dialog).getByRole("button", { name: "Commit" }));
      await waitFor(() => expect(within(dialog).getByRole("alert").textContent).toBe(refusal.message));
    }
    expect(within(dialog).getByRole("textbox", { name: "First line" })).toBeInTheDocument();
    expect(backend.calls("commitSlice")).toHaveLength(3);
  });

  it("shows a commitStatus failure as-is on the button", async () => {
    backend.fail("commitStatus", { code: "not_a_git_repo", message: "The project folder isn't a git repository." });
    backend.respond("agentList", []);
    const { sheet } = await openBlueprintSheet();
    expect(await within(sheet).findByLabelText("Commit changes: The project folder isn't a git repository.")).toBeInTheDocument();
  });

  it("asks once after Mark done in the slice sheet, and only opens the dialog", async () => {
    backend.respond("commitStatus", READY);
    backend.respond("agentList", []);
    const { sheet, user } = await openBlueprintSheet();
    backend.respondOnce("setSliceStatus", plan({ revision: 2, slices: [feed({ status: "done", ready: false })] }));

    const status = within(sheet).getByRole("group", { name: "Status" });
    await user.click(within(status).getByRole("button", { name: "done" }));

    const prompt = await within(sheet).findByText("Commit this slice?");
    await user.click(within(prompt.parentElement!).getByRole("button", { name: "Review commit" }));
    expect(await screen.findByRole("dialog", { name: "Commit “Feed”" })).toBeInTheDocument();
    // Once only: the prompt is gone, the plain button is back, and nothing was committed.
    // The open dialog makes the sheet inert to role queries, hence `hidden`.
    expect(within(sheet).queryByText("Commit this slice?")).not.toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: "Commit changes", hidden: true })).toBeInTheDocument();
    expect(backend.calls("commitSlice")).toHaveLength(0);
  });

  it("sits in the Verification sheet header, with the same prompt after Mark done", async () => {
    backend.respond("getPlan", plan({ revision: 1, slices: [feed()] }));
    backend.respond("getTestCommands", testCommands({ commands: ["npm test"] }));
    backend.respond("managerDigest", digest({ slices: [sliceDigest({ sliceId: "s-feed", name: "Feed" })] }));
    backend.respond("getTeam", { agents: [] });
    backend.respond("listReports", []);
    backend.respond("getVerification", null);
    backend.respond("commitStatus", READY);
    backend.respond("agentList", []);
    const { user } = renderWithApp(<Verification projectId={PROJECT_ID} />);

    await user.click(await screen.findByRole("button", { name: /Feed/ }));
    const sheet = await screen.findByRole("dialog");
    await waitFor(() => expect(within(sheet).getByRole("button", { name: "Commit changes" })).toBeEnabled());

    backend.respondOnce("setSliceStatus", plan({ revision: 2, slices: [feed({ status: "done", ready: false })] }));
    await user.click(within(sheet).getByRole("button", { name: "Mark done" }));
    const prompt = await within(sheet).findByText("Commit this slice?");
    expect(within(sheet).queryByRole("button", { name: "Commit changes" })).not.toBeInTheDocument();

    await user.click(within(prompt.parentElement!).getByRole("button", { name: "Review commit" }));
    const dialog = await screen.findByRole("dialog", { name: "Commit “Feed”" });
    expect(await within(dialog).findByRole("textbox", { name: "Body" })).toHaveValue(BODY);
    expect(backend.calls("commitSlice")).toHaveLength(0);
    expect(backend.unhandled).toEqual([]);
  });
});
