import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { HeadlineKind, Verification as Review, VerificationGuidance } from "@/lib/bindings";
import { backend } from "@/test/fake-backend";
import {
  PROJECT_ID,
  agentReport,
  commitStatus,
  digest,
  guidanceAction,
  guidedFinding,
  plan,
  session,
  slice,
  sliceDigest,
  teamAgent,
  testCommands,
  verification,
} from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Verification } from "./verification";

/** The page with slice s-feed (in progress, reported by Mark) and the given guidance; opens its sheet. */
async function openSheet(
  guidance: VerificationGuidance,
  { review = verification({ sliceId: "s-feed" }), sessions = [session("builder")] }: { review?: Review | null; sessions?: ReturnType<typeof session>[] } = {},
) {
  backend.respond("getPlan", plan({ slices: [slice({ id: "s-feed", name: "Feed", status: "in_progress" })] }));
  backend.respond("getTestCommands", testCommands({ commands: ["npm test"] }));
  backend.respond(
    "managerDigest",
    digest({ slices: [sliceDigest({ sliceId: "s-feed", name: "Feed", status: "in_progress", reportPresent: true, reportCount: 1 })] }),
  );
  backend.respond("getTeam", {
    agents: [
      teamAgent({ id: "builder", displayName: "Mark", title: "Frontend builder" }),
      teamAgent({ id: "qa", displayName: "Ana", title: "QA" }),
    ],
  });
  backend.respond("listReports", [agentReport("s-feed", "builder", { notes: "Built the feed." })]);
  backend.respond("getVerification", review);
  backend.respond("verificationGuidance", guidance);
  backend.respond("commitStatus", commitStatus({ files: [] }));
  backend.respond("agentList", sessions);

  const rendered = renderWithApp(<Verification projectId={PROJECT_ID} />);
  await rendered.user.click(await screen.findByRole("button", { name: /Feed/ }));
  const sheet = await screen.findByRole("dialog");
  const card = await within(sheet).findByRole("region", { name: "What to do" });
  return { ...rendered, sheet, card };
}

const HEADLINES: { kind: HeadlineKind; guidance: VerificationGuidance; buttons: string[]; text?: string }[] = [
  {
    kind: "passed",
    guidance: {
      headline: { kind: "passed", text: "Passed" },
      actions: [guidanceAction({ id: "mark_done", label: "Mark done", kind: "mark_done" }), guidanceAction({ id: "commit", label: "Commit the slice", kind: "commit" })],
      findings: [],
    },
    buttons: ["Mark done", "Commit the slice"],
  },
  {
    kind: "needs_fix",
    guidance: {
      headline: { kind: "needs_fix", text: "Needs a fix: 1 problem below." },
      actions: [
        guidanceAction({ id: "send:f0", label: "Send the failure to the agent (Mark)", kind: "send_to_agent", agentId: "builder", message: "npm test fails." }),
        guidanceAction({ id: "reverify", label: "Verify again", kind: "reverify" }),
      ],
      findings: [guidedFinding({ findingId: "f0", plain: "`npm test` failed (exit code 1).", group: "fix", actionIds: ["send:f0"] })],
    },
    buttons: ["Send the failure to the agent (Mark)", "Verify again"],
  },
  {
    kind: "safe_to_override",
    guidance: {
      headline: { kind: "safe_to_override", text: "Safe to override: what's left isn't this slice's problem." },
      actions: [
        guidanceAction({ id: "override", label: "Mark done with a reason", kind: "override", reasonTemplate: "Edit to a.ts needed; <reason>" }),
        guidanceAction({ id: "commit", label: "Commit the slice", kind: "commit" }),
      ],
      findings: [],
    },
    buttons: ["Mark done with a reason", "Commit the slice"],
  },
  {
    kind: "needs_decision",
    guidance: {
      headline: { kind: "needs_decision", text: "Needs your decision: see below." },
      actions: [
        guidanceAction({ id: "manual:f0", label: "Check src/merge.ts", kind: "manual", message: "Both slices changed src/merge.ts: open it and check the merged result." }),
        guidanceAction({ id: "reverify", label: "Verify again", kind: "reverify" }),
      ],
      findings: [],
    },
    buttons: ["Verify again"],
    text: "Both slices changed src/merge.ts: open it and check the merged result.",
  },
  {
    kind: "stale",
    guidance: {
      headline: { kind: "stale", text: "A report changed after this verification. Verify again." },
      actions: [guidanceAction({ id: "reverify", label: "Verify again", kind: "reverify" })],
      findings: [],
    },
    buttons: ["Verify again"],
  },
  {
    kind: "waiting",
    guidance: { headline: { kind: "waiting", text: "Waiting for Ana. Earlier results aren't final." }, actions: [], findings: [] },
    buttons: [],
  },
  {
    kind: "skipped",
    guidance: {
      headline: { kind: "skipped", text: "The setup files aren't committed." },
      actions: [
        guidanceAction({ id: "commit_setup", label: "Commit the setup", kind: "commit_setup" }),
        guidanceAction({ id: "restart_slice", label: "Restart the slice", kind: "manual", message: "Then move the slice back to queued and start it again." }),
      ],
      findings: [],
    },
    buttons: ["Commit the setup"],
    text: "Then move the slice back to queued and start it again.",
  },
];

describe("Verification guidance", () => {
  it.each(HEADLINES)("shows the $kind headline with its actions as buttons, and manual steps as text", async ({ guidance, buttons, text }) => {
    const { card } = await openSheet(guidance);
    expect(within(card).getByRole("status")).toHaveTextContent(guidance.headline.text);
    expect(within(card).queryAllByRole("button").map((button) => button.textContent)).toEqual(buttons);
    if (text) expect(within(card).getByText(text, { exact: false })).toBeInTheDocument();
    expect(backend.calls("verificationGuidance")).toEqual([[PROJECT_ID, "s-feed"]]);
    expect(backend.unhandled).toEqual([]);
  });

  it("says who it's waiting for and mutes everything below the card", async () => {
    const { sheet, card } = await openSheet(HEADLINES.find((entry) => entry.kind === "waiting")!.guidance);
    expect(within(card).getByRole("status")).toHaveTextContent("Waiting for Ana.");
    const rest = sheet.querySelector("[data-provisional]");
    expect(rest).toHaveClass("opacity-50");
    expect(within(rest as HTMLElement).getByText("Built the feed.")).toBeInTheDocument();
  });

  it("doesn't mute the rest for a final result", async () => {
    const { sheet } = await openSheet(HEADLINES[0].guidance);
    expect(sheet.querySelector("[data-provisional]")).toBeNull();
  });

  it("commits the setup from the card and shows what was committed", async () => {
    const { user, card } = await openSheet(HEADLINES.find((entry) => entry.kind === "skipped")!.guidance);
    backend.respond("commitSetup", { commit: "abc1234def", files: ["AGENTS.md", "agents/roles.json"] });
    await user.click(within(card).getByRole("button", { name: "Commit the setup" }));
    expect(await within(card).findByText("Committed 2 setup files as abc1234.")).toBeInTheDocument();
    expect(backend.calls("commitSetup")).toEqual([[PROJECT_ID]]);
  });

  describe("send to agent", () => {
    const needsFix = HEADLINES.find((entry) => entry.kind === "needs_fix")!.guidance;

    it("sends the prepared message as generated, pasted then Enter, and starts nothing", async () => {
      const { user, card } = await openSheet(needsFix);
      backend.respond("agentWrite", null);
      await user.click(within(card).getByRole("button", { name: "Send the failure to the agent (Mark)" }));

      // Shown first, editable, before anything is sent.
      const box = within(card).getByRole("textbox", { name: "Message to Mark" });
      expect(box).toHaveValue("npm test fails.");
      expect(backend.calls("agentWrite")).toHaveLength(0);

      await user.click(within(card).getByRole("button", { name: "Send to Mark" }));
      expect(await within(card).findByText("Sent to Mark.")).toBeInTheDocument();
      expect(backend.calls("agentWrite")).toEqual([
        [PROJECT_ID, "builder", "\x1b[200~npm test fails.\x1b[201~"],
        [PROJECT_ID, "builder", "\r"],
      ]);
      expect(backend.calls("agentStart")).toHaveLength(0);
      expect(within(card).queryByRole("textbox")).not.toBeInTheDocument();
    });

    it("sends the edited message instead, line breaks kept inside the paste", async () => {
      const { user, card } = await openSheet(needsFix);
      backend.respond("agentWrite", null);
      await user.click(within(card).getByRole("button", { name: "Send the failure to the agent (Mark)" }));
      const box = within(card).getByRole("textbox", { name: "Message to Mark" });
      await user.clear(box);
      await user.type(box, "Fix the feed test.{Shift>}{Enter}{/Shift}Keep the API as is.");
      await user.keyboard("{Control>}{Enter}{/Control}");

      await waitFor(() => expect(backend.calls("agentWrite")).toHaveLength(2));
      expect(backend.calls("agentWrite")[0]).toEqual([PROJECT_ID, "builder", "\x1b[200~Fix the feed test.\rKeep the API as is.\x1b[201~"]);
      expect(backend.calls("agentWrite")[1]).toEqual([PROJECT_ID, "builder", "\r"]);
    });

    it("says so when the agent has no live session, offers Copy, and sends nothing", async () => {
      const { user, card } = await openSheet(needsFix, { sessions: [session("builder", "exited")] });
      await user.click(within(card).getByRole("button", { name: "Send the failure to the agent (Mark)" }));
      expect(await within(card).findByText(/Mark has no live session/)).toBeInTheDocument();
      expect(within(card).getByRole("button", { name: "Send to Mark" })).toBeDisabled();
      expect(within(card).getByRole("button", { name: "Copy" })).toBeEnabled();
      expect(backend.calls("agentWrite")).toHaveLength(0);
      expect(backend.calls("agentStart")).toHaveLength(0);
    });

    it("shows a refused send as-is and keeps the message", async () => {
      const { user, card } = await openSheet(needsFix);
      backend.fail("agentWrite", { code: "not_found", message: "No running session for builder." });
      await user.click(within(card).getByRole("button", { name: "Send the failure to the agent (Mark)" }));
      await user.click(within(card).getByRole("button", { name: "Send to Mark" }));
      expect(await within(card).findByText("No running session for builder.")).toBeInTheDocument();
      expect(within(card).getByRole("textbox", { name: "Message to Mark" })).toHaveValue("npm test fails.");
    });
  });

  it("fills the override reason from a template, still editable, and names what is overridden", async () => {
    const guidance: VerificationGuidance = {
      headline: { kind: "safe_to_override", text: "Safe to override: what's left isn't this slice's problem." },
      actions: [
        guidanceAction({ id: "override", label: "Mark done with a reason", kind: "override", reasonTemplate: "Edit to src/a.ts needed for this slice; <reason>" }),
        guidanceAction({ id: "commit", label: "Commit the slice", kind: "commit" }),
      ],
      findings: [
        guidedFinding({ findingId: "f0", plain: "src/a.ts is outside this slice's files, but it's one small edit.", group: "override", actionIds: ["override"] }),
      ],
    };
    const { user, card } = await openSheet(guidance, {
      review: verification({
        sliceId: "s-feed",
        passed: false,
        findings: [{ severity: "warn", code: "outside_write_globs", message: "builder wrote src/a.ts", requirementId: null, path: "src/a.ts", agent: "builder" }],
      }),
    });
    await user.click(within(card).getByRole("button", { name: "Mark done with a reason" }));
    const dialog = await screen.findByRole("dialog", { name: "Mark it done anyway?" });
    expect(within(dialog).getByText("You are overriding:")).toBeInTheDocument();
    expect(within(dialog).getByText("src/a.ts is outside this slice's files, but it's one small edit.")).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: /Use a template/ }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit to src/a.ts needed for this slice; <reason>" }));
    const reason = within(dialog).getByRole("textbox", { name: "Reason" });
    expect(reason).toHaveValue("Edit to src/a.ts needed for this slice; <reason>");

    // Still editable: the user writes their own words over the placeholder.
    await user.clear(reason);
    await user.type(reason, "Edit to src/a.ts needed for this slice; the shared header import moved.");
    backend.respond("setSliceStatus", plan({ slices: [slice({ id: "s-feed", name: "Feed", status: "done" })], revision: 2 }));
    await user.click(within(dialog).getByRole("button", { name: "Mark done anyway" }));
    await waitFor(() => expect(backend.calls("setSliceStatus")).toHaveLength(1));
    expect(backend.calls("setSliceStatus")[0][3]).toEqual({ override: true, reason: "Edit to src/a.ts needed for this slice; the shared header import moved." });
  });

  it("groups findings by what they need, collapses info and ignore under Show N more, and keeps raw details in an expander", async () => {
    const guidance: VerificationGuidance = {
      headline: { kind: "needs_fix", text: "Needs a fix: 1 problem below." },
      actions: [guidanceAction({ id: "reverify", label: "Verify again", kind: "reverify" })],
      findings: [
        guidedFinding({ findingId: "f0", plain: "The empty state isn't reported.", whatToDo: "Ask Mark for it.", group: "fix" }),
        guidedFinding({ findingId: "f1", plain: "Mark left a note.", whatToDo: "Nothing to do.", group: "info" }),
        guidedFinding({ findingId: "f2", plain: "A lockfile changed.", whatToDo: "Nothing to do.", group: "ignore" }),
      ],
    };
    const review = verification({
      sliceId: "s-feed",
      passed: false,
      findings: [
        { severity: "danger", code: "unreported_requirement", message: "No report mentions r-empty.", requirementId: "r-empty", path: null, agent: null },
        { severity: "info", code: "report_note", message: "A note.", requirementId: null, path: null, agent: "builder" },
        { severity: "info", code: "lockfile_changed", message: "package-lock.json changed.", requirementId: null, path: "package-lock.json", agent: null },
      ],
      testRuns: [
        { command: "npm run lint", exitCode: 0, durationMs: 900, tail: "lint ok", timedOut: false },
        { command: "npm test", exitCode: 1, durationMs: 4200, tail: "1 failed: feed.test.ts", timedOut: false },
      ],
    });
    const { user, sheet } = await openSheet(guidance, { review });

    const fix = await within(sheet).findByRole("group", { name: "To fix" });
    expect(within(fix).getByText("The empty state isn't reported.")).toBeVisible();
    expect(within(fix).getByText("Ask Mark for it.")).toBeVisible();
    expect(within(sheet).queryByText("Mark left a note.")).not.toBeInTheDocument();
    expect(within(sheet).queryByText("A lockfile changed.")).not.toBeInTheDocument();

    // The raw code stays one click away.
    expect(within(fix).queryByText("unreported_requirement")).not.toBeInTheDocument();
    await user.click(within(fix).getByRole("button", { name: "Details" }));
    expect(within(fix).getByText("unreported_requirement")).toBeVisible();
    expect(within(fix).getByText("No report mentions r-empty.")).toBeVisible();

    await user.click(within(sheet).getByRole("button", { name: "Show 2 more" }));
    expect(within(sheet).getByRole("group", { name: "For information" })).toHaveTextContent("Mark left a note.");
    expect(within(sheet).getByRole("group", { name: "Nothing to do" })).toHaveTextContent("A lockfile changed.");

    // Test runs: opened by the failing run, which shows its output; the passing one stays shut.
    expect(within(sheet).getByText("1 of 2 failed")).toBeInTheDocument();
    expect(within(sheet).getByText("1 failed: feed.test.ts")).toBeVisible();
    expect(within(sheet).queryByText("lint ok")).not.toBeInTheDocument();
  });

  it("keeps Test runs collapsed when every run passed", async () => {
    const review = verification({
      sliceId: "s-feed",
      testRuns: [{ command: "npm test", exitCode: 0, durationMs: 900, tail: "12 passed", timedOut: false }],
    });
    const { user, sheet } = await openSheet(HEADLINES[0].guidance, { review });
    const toggle = within(sheet).getByRole("button", { name: /Test runs/ });
    expect(within(sheet).queryByText("npm test")).not.toBeInTheDocument();
    await user.click(toggle);
    expect(within(sheet).getByText("npm test")).toBeVisible();
  });
});
