import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend, type EventPayload } from "@/test/fake-backend";
import { PROJECT_ID, agentReport, commitStatus, digest, plan, slice, sliceDigest, teamAgent, testCommands } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Verification } from "./verification";

/** An agent's `report_ready` for slice s-feed. */
function reportReady(id: number, agentId: string): EventPayload<"agentEvent"> {
  return {
    id,
    ts: 0,
    projectId: PROJECT_ID,
    agentId,
    sliceId: "s-feed",
    kind: "report_ready",
    severity: "info",
    title: "Report ready",
    detail: null,
    path: null,
    command: null,
  };
}

describe("Verification widget", () => {
  it("counts reports from the digest and opens the sheet on the newest report, with each report's time", async () => {
    backend.respond(
      "getPlan",
      plan({ slices: [slice({ id: "s-feed", name: "Feed" }), slice({ id: "s-auth", name: "Auth", layer: 1 })] }),
    );
    backend.respond("getTestCommands", testCommands({ commands: ["npm test"] }));
    backend.respond(
      "managerDigest",
      digest({
        slices: [
          sliceDigest({ sliceId: "s-feed", name: "Feed", reportPresent: true, reportCount: 2 }),
          sliceDigest({ sliceId: "s-auth", name: "Auth", reportPresent: true, reportCount: 1 }),
        ],
      }),
    );
    backend.respond("getTeam", {
      agents: [
        teamAgent({ id: "builder", displayName: "Mark", title: "Frontend builder" }),
        teamAgent({ id: "qa", displayName: "Ana", title: "QA" }),
      ],
    });
    // Listed builder first, but QA's report was written later.
    const builder = { ...agentReport("s-feed", "builder", { notes: "Built the feed list." }), writtenAt: "2026-10-01T09:00:00.000Z" };
    const qa = { ...agentReport("s-feed", "qa", { notes: "Checked empty and error states." }), writtenAt: "2026-10-01T15:30:00.000Z" };
    backend.respond("listReports", [builder, qa]);
    backend.respond("getVerification", null);
    backend.respond("verificationGuidance", { headline: { kind: "stale", text: "Not verified yet." }, actions: [], findings: [] });
    backend.respond("commitStatus", commitStatus());
    backend.respond("agentList", []);

    const { user } = renderWithApp(<Verification projectId={PROJECT_ID} />);

    // The count comes from the digest's reportCount: no per-slice listReports.
    const feedRow = await screen.findByRole("button", { name: /Feed/ });
    expect(await within(feedRow).findByText("2 reports")).toBeInTheDocument();
    const authRow = screen.getByRole("button", { name: /Auth/ });
    expect(within(authRow).getByText("Report")).toBeInTheDocument();
    expect(within(authRow).queryByText(/reports/)).not.toBeInTheDocument();
    expect(backend.calls("listReports")).toHaveLength(0);

    await user.click(feedRow);
    const sheet = await screen.findByRole("dialog");
    expect(await within(sheet).findByText("Reports")).toBeInTheDocument();

    // One tab per report, named from the team, each with its time.
    const tabs = within(sheet).getByRole("tablist", { name: "Reports by agent" });
    const [markTab, anaTab] = within(tabs).getAllByRole("tab");
    expect(markTab).toHaveTextContent("Mark");
    expect(markTab).toHaveTextContent("Frontend builder");
    expect(within(markTab).getByText((_, element) => element?.getAttribute("datetime") === builder.writtenAt)).toBeInTheDocument();
    expect(within(anaTab).getByText((_, element) => element?.getAttribute("datetime") === qa.writtenAt)).toBeInTheDocument();

    // Opens on the newest (Ana's), not the first listed; the other is one click away.
    expect(anaTab).toHaveAttribute("aria-selected", "true");
    expect(within(sheet).getByText("Checked empty and error states.")).toBeVisible();
    await user.click(markTab);
    expect(await within(sheet).findByText("Built the feed list.")).toBeVisible();

    expect(backend.calls("listReports")).toEqual([[PROJECT_ID, "s-feed"]]);
    expect(backend.calls("verifySlice")).toHaveLength(0); // opening never verifies
    expect(backend.unhandled).toEqual([]);
  });

  it("switches to a report written while the sheet is open, unless a tab was picked by hand", async () => {
    backend.respond("getPlan", plan({ slices: [slice({ id: "s-feed", name: "Feed" })] }));
    backend.respond("getTestCommands", testCommands({ commands: ["npm test"] }));
    backend.respond(
      "managerDigest",
      digest({ slices: [sliceDigest({ sliceId: "s-feed", name: "Feed", reportPresent: true, reportCount: 2 })] }),
    );
    backend.respond("getTeam", { agents: [] });
    backend.respond("getVerification", null);
    backend.respond("verificationGuidance", { headline: { kind: "stale", text: "Not verified yet." }, actions: [], findings: [] });
    backend.respond("commitStatus", commitStatus());
    backend.respond("agentList", []);
    const builder = { ...agentReport("s-feed", "builder", { notes: "First pass." }), writtenAt: "2026-10-01T15:00:00.000Z" };
    const qa = { ...agentReport("s-feed", "qa", { notes: "QA pass." }), writtenAt: "2026-10-01T09:00:00.000Z" };
    backend.respondOnce("listReports", [builder, qa]);

    const { user } = renderWithApp(<Verification projectId={PROJECT_ID} />);
    await user.click(await screen.findByRole("button", { name: /Feed/ }));
    const sheet = await screen.findByRole("dialog");
    expect(await within(sheet).findByText("First pass.")).toBeVisible();

    // QA rewrites its report: it's now the newest, so the sheet follows it.
    backend.respondOnce("listReports", [builder, { ...qa, report: { ...qa.report, notes: "QA again." }, writtenAt: "2026-10-01T16:00:00.000Z" }]);
    backend.emit("agentEvent", reportReady(1, "qa"));
    expect(await within(sheet).findByText("QA again.")).toBeVisible();

    // Picked by hand: a newer report elsewhere doesn't take the tab away.
    const [builderTab] = within(sheet).getAllByRole("tab");
    await user.click(builderTab);
    backend.respondOnce("listReports", [{ ...builder, writtenAt: "2026-10-01T15:00:00.000Z" }, { ...qa, writtenAt: "2026-10-01T17:00:00.000Z" }]);
    backend.emit("agentEvent", reportReady(2, "qa"));
    await waitFor(() => expect(backend.calls("listReports")).toHaveLength(3));
    expect(within(sheet).getByText("First pass.")).toBeVisible();
    expect(backend.unhandled).toEqual([]);
  });
});
