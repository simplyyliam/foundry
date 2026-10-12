import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, agentReport, digest, plan, slice, sliceDigest, teamAgent, testCommands } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Verification } from "./verification";

describe("Verification widget", () => {
  it("shows both reports of a slice that has two", async () => {
    backend.respond("getPlan", plan({ slices: [slice({ id: "s-feed", name: "Feed" })] }));
    backend.respond("getTestCommands", testCommands({ commands: ["npm test"] }));
    backend.respond(
      "managerDigest",
      digest({ slices: [sliceDigest({ sliceId: "s-feed", name: "Feed", reportPresent: true, reportCount: 2 })] }),
    );
    backend.respond("getTeam", {
      agents: [
        teamAgent({ id: "builder", displayName: "Mark", title: "Frontend builder" }),
        teamAgent({ id: "qa", displayName: "Ana", title: "QA" }),
      ],
    });
    backend.respond("listReports", [
      agentReport("s-feed", "builder", { notes: "Built the feed list." }),
      agentReport("s-feed", "qa", { notes: "Checked empty and error states." }),
    ]);
    backend.respond("getVerification", null);

    const { user } = renderWithApp(<Verification projectId={PROJECT_ID} />);

    // The row's Report chip counts both (from listReports, per slice with a report).
    const row = await screen.findByRole("button", { name: /Feed/ });
    expect(await within(row).findByText("s: 2")).toBeInTheDocument(); // read as "Reports: 2"

    await user.click(row);
    const sheet = await screen.findByRole("dialog");
    expect(await within(sheet).findByText("Reports")).toBeInTheDocument();
    expect(within(sheet).getByText("2 reports", { exact: false })).toBeInTheDocument();

    // One tab per report, named from the team.
    const tabs = within(sheet).getByRole("tablist", { name: "Reports by agent" });
    const [markTab, anaTab] = within(tabs).getAllByRole("tab");
    expect(markTab).toHaveTextContent("Mark");
    expect(markTab).toHaveTextContent("Frontend builder");
    expect(anaTab).toHaveTextContent("Ana");

    // Opens on the first listed report; the other is one click away.
    expect(within(sheet).getByText("Built the feed list.")).toBeVisible();
    await user.click(anaTab);
    expect(await within(sheet).findByText("Checked empty and error states.")).toBeVisible();

    expect(backend.calls("listReports")).toContainEqual([PROJECT_ID, "s-feed"]);
    expect(backend.calls("verifySlice")).toHaveLength(0); // opening never verifies
    expect(backend.unhandled).toEqual([]);
  });
});
