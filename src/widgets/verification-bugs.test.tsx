import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AgentReport, Bug, FixPrompt } from "@/lib/bindings";
import { backend, type EventPayload } from "@/test/fake-backend";
import {
  PROJECT_ID,
  agentReport,
  commitStatus,
  digest,
  plan,
  requirement,
  session,
  slice,
  sliceDigest,
  teamAgent,
  testCommands,
} from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Verification } from "./verification";

function bug(overrides: Partial<Bug> & Pick<Bug, "id" | "title">): Bug {
  return { severity: "medium", requirementIds: [], steps: [], failingTests: [], suspectedLocations: [], ownerHint: null, ...overrides };
}

/** An agent's `report_ready` for slice s-feed. */
function reportReady(id: number, agentId: string): EventPayload<"agentEvent"> {
  return { id, ts: 0, projectId: PROJECT_ID, agentId, sliceId: "s-feed", kind: "report_ready", severity: "info", title: "Report ready", detail: null, path: null, command: null };
}

const at = (writtenAt: string, report: AgentReport): AgentReport => ({ ...report, writtenAt });

const EMPTY_FEED = bug({
  id: "b-1",
  title: "Empty feed shows a spinner forever",
  severity: "high",
  requirementIds: ["r-empty"],
  steps: ["Sign in as a new user", "Open the feed"],
  failingTests: [{ file: "src/feed/feed.test.tsx", name: "shows the empty state" }],
});
const SLOW_AVATAR = bug({ id: "b-2", title: "Avatars load late", severity: "low", requirementIds: ["r-avatar"] });

const PROMPTS: FixPrompt[] = [
  { agentId: "builder", agentName: "Mark", bugIds: ["b-1"], prompt: "You are Mark.\n\n## Fix 1 bug in slice: Feed" },
  { agentId: "api", agentName: "Lea", bugIds: ["b-2"], prompt: "You are Lea.\n\n## Fix 1 bug in slice: Feed" },
];

/** The page with slice s-feed and the given reports; opens its sheet. */
async function openSheet(reports: AgentReport[], { prompts = PROMPTS, sessions = [session("builder"), session("api")] } = {}) {
  backend.respond(
    "getPlan",
    plan({
      slices: [slice({ id: "s-feed", name: "Feed", status: "in_progress" })],
      requirements: [
        requirement({ id: "r-empty", text: "An empty feed says so", sliceId: "s-feed" }),
        requirement({ id: "r-avatar", text: "Avatars show", sliceId: "s-feed" }),
      ],
    }),
  );
  backend.respond("getTestCommands", testCommands({ commands: ["npm test"] }));
  backend.respond(
    "managerDigest",
    digest({ slices: [sliceDigest({ sliceId: "s-feed", name: "Feed", status: "in_progress", reportPresent: true, reportCount: reports.length })] }),
  );
  backend.respond("getTeam", {
    agents: [
      teamAgent({ id: "builder", displayName: "Mark", title: "Frontend builder" }),
      teamAgent({ id: "api", displayName: "Lea", title: "API builder" }),
      teamAgent({ id: "qa", displayName: "Ana", title: "QA" }),
    ],
  });
  backend.respond("listReports", reports);
  backend.respond("getVerification", null);
  backend.respond("verificationGuidance", { headline: { kind: "needs_fix", text: "Needs a fix." }, actions: [], findings: [] });
  backend.respond("commitStatus", commitStatus({ files: [] }));
  backend.respond("agentList", sessions);
  backend.respond("generateFixPrompts", prompts);

  const rendered = renderWithApp(<Verification projectId={PROJECT_ID} />);
  await rendered.user.click(await screen.findByRole("button", { name: /Feed/ }));
  const sheet = await screen.findByRole("dialog");
  await within(sheet).findByRole("region", { name: "What to do" });
  return { ...rendered, sheet };
}

const qaReport = (bugs: Bug[], writtenAt = "2026-10-01T12:00:00.000Z") => at(writtenAt, agentReport("s-feed", "qa", { bugs }));

describe("Verification bugs and fix prompts", () => {
  it("shows neither section when no report lists bugs, and prepares no prompts", async () => {
    const { sheet } = await openSheet([agentReport("s-feed", "builder", { notes: "Built it." }), qaReport([])]);
    // The reports have loaded (two of them), and still nothing about bugs.
    await within(sheet).findByRole("heading", { name: "Reports" });
    expect(within(sheet).queryByRole("region", { name: "Bugs found" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("region", { name: "Fix prompts" })).not.toBeInTheDocument();
    expect(backend.calls("generateFixPrompts")).toHaveLength(0);
    expect(backend.unhandled).toEqual([]);
  });

  it("lists each bug with severity in words, requirements, steps, failing tests and its owner", async () => {
    const { sheet } = await openSheet([agentReport("s-feed", "builder"), qaReport([SLOW_AVATAR, EMPTY_FEED])]);
    const section = await within(sheet).findByRole("region", { name: "Bugs found" });
    expect(section).toHaveTextContent("From Ana's run");

    const rows = within(section).getAllByRole("article");
    // Most severe first.
    expect(rows.map((row) => row.getAttribute("aria-label"))).toEqual([EMPTY_FEED.title, SLOW_AVATAR.title]);
    const empty = rows[0];
    expect(within(empty).getByText("High")).toBeInTheDocument();
    expect(within(empty).getByText("An empty feed says so")).toBeInTheDocument();
    expect(within(empty).getByText("Open the feed")).toBeInTheDocument();
    expect(within(empty).getByText("shows the empty state")).toBeInTheDocument();
    expect(within(empty).getByText("src/feed/feed.test.tsx")).toBeInTheDocument();
    expect(await within(empty).findByText("Mark")).toBeInTheDocument();
    expect(within(rows[1]).getByText("Low")).toBeInTheDocument();
    expect(await within(rows[1]).findByText("Lea")).toBeInTheDocument();
    expect(backend.calls("generateFixPrompts")).toEqual([[PROJECT_ID, "s-feed"]]);
  });

  it("shows one fix prompt card per agent with its name, bug count and text", async () => {
    const { sheet } = await openSheet([qaReport([EMPTY_FEED, SLOW_AVATAR])], {
      prompts: [{ ...PROMPTS[0], bugIds: ["b-1", "b-2"] }, PROMPTS[1]],
    });
    const section = await within(sheet).findByRole("region", { name: "Fix prompts" });
    const cards = await within(section).findAllByRole("article");
    expect(cards.map((card) => card.getAttribute("aria-label"))).toEqual(["Fix prompt for Mark", "Fix prompt for Lea"]);
    expect(within(cards[0]).getByText("2 bugs")).toBeInTheDocument();
    expect(within(cards[1]).getByText("1 bug")).toBeInTheDocument();
    expect(within(cards[0]).getByLabelText("Prompt for Mark")).toHaveTextContent("## Fix 1 bug in slice: Feed");
    expect(within(cards[0]).getByRole("button", { name: "Send to Mark" })).toBeInTheDocument();
    expect(within(cards[1]).getByRole("button", { name: "Send to Lea" })).toBeInTheDocument();
  });

  it("shows the backend's refusal as-is when prompts can't be prepared, and the report's owner hint", async () => {
    backend.failOnce("generateFixPrompts", { code: "invalid_input", message: "bug b-1 has no owner: no agent's write paths match its locations" });
    const { sheet } = await openSheet([qaReport([{ ...EMPTY_FEED, ownerHint: "builder" }])]);
    const section = await within(sheet).findByRole("region", { name: "Fix prompts" });
    expect(await within(section).findByText("bug b-1 has no owner: no agent's write paths match its locations")).toBeInTheDocument();
    const bugs = within(sheet).getByRole("region", { name: "Bugs found" });
    expect(within(bugs).getByText("(the report's suggestion)")).toBeInTheDocument();
  });

  it("copies the prompt as generated", async () => {
    const { user, sheet } = await openSheet([qaReport([EMPTY_FEED])]);
    const card = await within(sheet).findByRole("article", { name: "Fix prompt for Mark" });
    await user.click(within(card).getByRole("button", { name: "Copy" }));
    expect(await within(card).findByRole("button", { name: "Copied" })).toBeInTheDocument();
    expect(await navigator.clipboard.readText()).toBe(PROMPTS[0].prompt);
  });

  it("sends the edited prompt to the agent's live session, pasted then Enter, and starts nothing", async () => {
    const { user, sheet } = await openSheet([qaReport([EMPTY_FEED])]);
    backend.respond("agentWrite", null);
    const card = await within(sheet).findByRole("article", { name: "Fix prompt for Mark" });
    await user.click(within(card).getByRole("button", { name: "Send to Mark" }));

    // Editable first; nothing sent yet.
    const box = within(card).getByRole("textbox", { name: "Message to Mark" });
    expect(box).toHaveValue(PROMPTS[0].prompt);
    expect(backend.calls("agentWrite")).toHaveLength(0);

    await user.type(box, "{Shift>}{Enter}{/Shift}Keep the API as is.");
    await user.click(within(card).getByRole("button", { name: "Send to Mark" }));
    expect(await within(card).findByText("Sent to Mark.")).toBeInTheDocument();
    expect(backend.calls("agentWrite")).toEqual([
      [PROJECT_ID, "builder", "\x1b[200~You are Mark.\r\r## Fix 1 bug in slice: Feed\rKeep the API as is.\x1b[201~"],
      [PROJECT_ID, "builder", "\r"],
    ]);
    expect(backend.calls("agentStart")).toHaveLength(0);
  });

  it("won't send without a live session and offers Copy instead", async () => {
    const { user, sheet } = await openSheet([qaReport([EMPTY_FEED])], { sessions: [session("builder", "exited")] });
    const card = await within(sheet).findByRole("article", { name: "Fix prompt for Mark" });
    await user.click(within(card).getByRole("button", { name: "Send to Mark" }));
    expect(await within(card).findByText(/Mark has no live session/)).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Send to Mark" })).toBeDisabled();
    expect(within(card).getByRole("button", { name: "Copy" })).toBeEnabled();
    expect(backend.calls("agentWrite")).toHaveLength(0);
  });

  it("shows only the latest run's bugs, with earlier runs behind a quiet expander", async () => {
    const older = at("2026-10-01T09:00:00.000Z", agentReport("s-feed", "tester", { bugs: [SLOW_AVATAR] }));
    const { user, sheet } = await openSheet([older, qaReport([EMPTY_FEED], "2026-10-01T15:00:00.000Z")]);
    const section = await within(sheet).findByRole("region", { name: "Bugs found" });
    expect(within(section).getByRole("article", { name: EMPTY_FEED.title })).toBeInTheDocument();
    expect(within(section).queryByRole("article", { name: SLOW_AVATAR.title })).not.toBeInTheDocument();

    await user.click(within(section).getByRole("button", { name: /Earlier runs/ }));
    expect(within(section).getByRole("article", { name: SLOW_AVATAR.title })).toBeVisible();
  });

  it("never marks a bug fixed: the owner's report changes nothing, QA's next run shows as it is", async () => {
    const { sheet } = await openSheet([qaReport([EMPTY_FEED, SLOW_AVATAR])]);
    const section = await within(sheet).findByRole("region", { name: "Bugs found" });
    expect(within(section).getAllByRole("article")).toHaveLength(2);

    // The owner reports (newer, no bugs listed): QA's bugs stay exactly as listed.
    backend.respond("listReports", [qaReport([EMPTY_FEED, SLOW_AVATAR]), at("2026-10-02T10:00:00.000Z", agentReport("s-feed", "builder", { bugs: [] }))]);
    backend.emit("agentEvent", reportReady(1, "builder"));
    await waitFor(() => expect(backend.calls("listReports").length).toBeGreaterThan(1));
    expect(within(section).getAllByRole("article")).toHaveLength(2);
    expect(within(sheet).queryByText(/fixed/i)).not.toBeInTheDocument();

    // QA's next run lists only one: that's what shows, with no "fixed" label on the other.
    backend.respond("listReports", [qaReport([SLOW_AVATAR], "2026-10-02T12:00:00.000Z")]);
    backend.emit("agentEvent", reportReady(2, "qa"));
    await waitFor(() => expect(within(section).getAllByRole("article")).toHaveLength(1));
    expect(within(section).getByRole("article", { name: SLOW_AVATAR.title })).toBeInTheDocument();
    expect(within(sheet).queryByText(/fixed/i)).not.toBeInTheDocument();
    await waitFor(() => expect(backend.calls("generateFixPrompts").length).toBeGreaterThanOrEqual(3));
  });
});
