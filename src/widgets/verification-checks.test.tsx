import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { CheckResult } from "@/lib/bindings";
import { backend } from "@/test/fake-backend";
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
  verification,
} from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Verification } from "./verification";

async function openSheet(checkResults: CheckResult[] | undefined) {
  backend.respond(
    "getPlan",
    plan({
      slices: [slice({ id: "s-feed", name: "Feed", status: "in_progress" })],
      requirements: [
        requirement({ id: "r-ui", text: "Use shadcn on Base UI", kind: "constraint", sliceId: "s-feed" }),
        requirement({ id: "r-rule", text: "No inline colours", kind: "constraint", projectWide: true }),
      ],
    }),
  );
  backend.respond("getTestCommands", testCommands({ commands: ["npm test"] }));
  backend.respond(
    "managerDigest",
    digest({ slices: [sliceDigest({ sliceId: "s-feed", name: "Feed", status: "in_progress", reportPresent: true, reportCount: 1 })] }),
  );
  backend.respond("getTeam", { agents: [teamAgent({ id: "builder", displayName: "Mark", title: "Frontend builder" })] });
  backend.respond("listReports", [agentReport("s-feed", "builder")]);
  backend.respond("getVerification", verification({ sliceId: "s-feed", passed: false, checkResults }));
  backend.respond("verificationGuidance", { headline: { kind: "needs_fix", text: "Needs a fix." }, actions: [], findings: [] });
  backend.respond("commitStatus", commitStatus());
  backend.respond("agentList", [session("builder")]);

  const rendered = renderWithApp(<Verification projectId={PROJECT_ID} />);
  await rendered.user.click(await screen.findByRole("button", { name: /Feed/ }));
  const sheet = await screen.findByRole("dialog");
  await within(sheet).findByRole("region", { name: "What to do" });
  return { ...rendered, sheet };
}

describe("Verification check results", () => {
  it("lists each requirement's checks with pass or fail as words and the detail of a failure", async () => {
    const { sheet } = await openSheet([
      { requirementId: "r-ui", check: { type: "file_exists", path: "components.json" }, passed: true, detail: "components.json exists" },
      {
        requirementId: "r-ui",
        check: { type: "dependency_absent", name: "@radix-ui/*" },
        passed: false,
        detail: "@radix-ui/react-dialog is in package.json:12",
      },
      { requirementId: "r-rule", check: { type: "text_absent", glob: "src/**/*.tsx", pattern: "#[0-9a-f]{6}" }, passed: true, detail: "no match" },
    ]);

    const section = await within(sheet).findByRole("region", { name: "Requirement checks" });
    expect(within(section).getByText("1 of 3 failed")).toBeInTheDocument();

    const ui = within(section).getByRole("list", { name: /Use shadcn on Base UI/ });
    const [passed, failed] = within(ui).getAllByRole("listitem");
    expect(passed).toHaveTextContent("PassedFile components.json must exist");
    // A pass doesn't repeat its detail.
    expect(passed).not.toHaveTextContent("components.json exists");
    expect(failed).toHaveTextContent("FailedDependency @radix-ui/* must not be present");
    expect(failed).toHaveTextContent("@radix-ui/react-dialog is in package.json:12");

    const rule = within(section).getByRole("list", { name: /No inline colours/ });
    expect(within(rule).getByRole("listitem")).toHaveTextContent("PassedNo match for #[0-9a-f]{6} in src/**/*.tsx");
    expect(backend.unhandled).toEqual([]);
  });

  it("shows no checks section for a review without check results", async () => {
    const { sheet } = await openSheet(undefined);
    expect(await within(sheet).findByText("Test runs")).toBeInTheDocument();
    expect(within(sheet).queryByRole("region", { name: "Requirement checks" })).not.toBeInTheDocument();
  });
});
