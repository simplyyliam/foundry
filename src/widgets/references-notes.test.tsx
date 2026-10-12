import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { BakeryItem, BakeryState } from "@/lib/bindings";
import { SidebarProvider, resetPanelRequests } from "@/shared";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { References as ReferencesPage } from "@/features/references";
import { ContextualPanelFrame } from "@/pages/app/layout/contextual-panel/ContextualPanelFrame";
import { Prompts } from "./prompts";
import { ReferencesPanel } from "./references";

// jsdom has no Tauri: image URLs pass through, and file drops never arrive.
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (path: string) => path }));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }),
}));

afterEach(resetPanelRequests);

function item(overrides: Partial<BakeryItem> & Pick<BakeryItem, "id" | "file">): BakeryItem {
  return { label: "", take: "", skip: "", tags: [], appliesTo: [], x: 0, y: 0, w: 1, h: 1, z: 0, ...overrides };
}

function bakery(items: BakeryItem[]): BakeryState {
  return { items, unlabeledCount: 0, maxImages: 40, dir: "C:/p/docs/bakery" };
}

const door = item({ id: "b-1", file: "01-red-door.png", label: "Red door", take: "The warm red", skip: "The brick wall" });

async function openSheet(items: BakeryItem[]) {
  backend.respond("bakeryList", bakery(items));
  const rendered = renderWithApp(
    <SidebarProvider>
      <ReferencesPage projectId={PROJECT_ID} layout="panel" />
    </SidebarProvider>,
  );
  await rendered.user.click(await screen.findByRole("button", { name: "Open Red door" }));
  const sheet = await screen.findByRole("dialog");
  return { ...rendered, sheet };
}

describe("Bakery entry notes", () => {
  it("explains Take and Skip under their fields", async () => {
    const { sheet } = await openSheet([door]);
    expect(within(sheet).getByRole("textbox", { name: "Take" })).toHaveAccessibleDescription("What to copy");
    expect(within(sheet).getByRole("textbox", { name: "Skip" })).toHaveAccessibleDescription(
      "What to ignore: the more specific, the less the agent guesses",
    );
  });

  it("asks before saving an empty Skip, and Save anyway saves it", async () => {
    const { user, sheet } = await openSheet([door]);
    const skip = within(sheet).getByRole("textbox", { name: "Skip" });

    await user.clear(skip);
    await user.tab();

    const question = await within(sheet).findByRole("alert");
    expect(question).toHaveTextContent("Skip is empty, so agents may copy everything. Save anyway?");
    expect(backend.calls("bakeryUpdate")).toHaveLength(0);

    backend.respondOnce("bakeryUpdate", bakery([{ ...door, skip: "" }]));
    await user.click(within(question).getByRole("button", { name: "Save anyway" }));

    await waitFor(() =>
      expect(backend.calls("bakeryUpdate")).toEqual([
        [PROJECT_ID, "b-1", { label: null, take: null, skip: "", tags: null, appliesTo: null }],
      ]),
    );
    expect(within(sheet).queryByText(/Skip is empty/)).not.toBeInTheDocument();
  });

  it("Keep editing goes back to the field without saving", async () => {
    const { user, sheet } = await openSheet([door]);
    const skip = within(sheet).getByRole("textbox", { name: "Skip" });

    await user.clear(skip);
    await user.tab();
    await user.click(await within(sheet).findByRole("button", { name: "Keep editing" }));

    expect(skip).toHaveFocus();
    expect(within(sheet).queryByText(/Skip is empty/)).not.toBeInTheDocument();
    await user.type(skip, "The sky");
    backend.respondOnce("bakeryUpdate", bakery([{ ...door, skip: "The sky" }]));
    await user.tab();
    await waitFor(() => expect(backend.calls("bakeryUpdate")).toHaveLength(1));
    expect(backend.calls("bakeryUpdate")[0][2].skip).toBe("The sky");
  });

  it("saves a non-empty Skip without asking", async () => {
    const { user, sheet } = await openSheet([door]);
    const skip = within(sheet).getByRole("textbox", { name: "Skip" });

    backend.respondOnce("bakeryUpdate", bakery([{ ...door, skip: "The brick wall and the plant" }]));
    await user.type(skip, " and the plant");
    await user.tab();

    await waitFor(() => expect(backend.calls("bakeryUpdate")).toHaveLength(1));
    expect(within(sheet).queryByText(/Skip is empty/)).not.toBeInTheDocument();
  });
});

describe("Prompt warning for incomplete Bakery notes", () => {
  it("names each entry under the prompt and opens its editor in the References panel", async () => {
    const panels = item({ id: "b-2", file: "02-panels.png", label: "Panels", take: "Card spacing" });
    backend.respond("bakeryList", bakery([door, panels]));
    backend.respond("listPrompts", []);
    backend.respond("getPlan", { revision: 1, slices: [], requirements: [], deferred: [], openQuestions: [], unresolvedConflicts: 0 });
    backend.respond("getTeam", null);
    backend.respond("generateHandoff", {
      path: ".foundry/prompts/handoff.md",
      warnings: ["2 Bakery images have no label", "bakery_note_incomplete: 02-panels.png (no Skip)"],
      bakeryNoteIssues: [{ entryId: "b-2", file: "02-panels.png", missing: "skip" }],
    });

    const { user } = renderWithApp(
      <SidebarProvider>
        <Prompts projectId={PROJECT_ID} />
        <ReferencesPanel projectId={PROJECT_ID} />
      </SidebarProvider>,
    );
    await user.click(await screen.findByRole("button", { name: "Generate hand-off" }));

    const entries = await screen.findByRole("list", { name: "References with incomplete notes" });
    expect(entries.closest("[role=status]")).toHaveTextContent("A reference in this prompt has incomplete notes");
    // The structured warning replaces the raw one; other warnings stay as-is.
    expect(screen.queryByText(/bakery_note_incomplete/)).not.toBeInTheDocument();
    expect(screen.getByText("2 Bakery images have no label")).toBeInTheDocument();

    await user.click(within(entries).getByRole("button", { name: "Edit 02-panels.png (no Skip)" }));

    const sheet = await screen.findByRole("dialog");
    expect(within(sheet).getByText("02-panels.png")).toBeInTheDocument();
    expect(within(sheet).getByRole("textbox", { name: "Skip" })).toHaveValue("");
  });

  it("switches the panel from Roadmap to References to open the entry", async () => {
    const panels = item({ id: "b-2", file: "02-panels.png", label: "Panels", take: "Card spacing" });
    backend.respond("bakeryList", bakery([door, panels]));
    backend.respond("listPrompts", []);
    backend.respond("getPlan", { revision: 1, slices: [], requirements: [], deferred: [], openQuestions: [], unresolvedConflicts: 0 });
    backend.respond("listRoadmapConflicts", []);
    backend.respond("getRoadmap", null);
    backend.respond("getDocument", "Brain dump.");
    backend.respond("claudeStatus", { path: "claude", version: null, loggedIn: true, authMethod: null, apiProvider: null, subscriptionType: null, problem: null });
    backend.respond("getTeam", null);
    backend.respond("roleHandbookList", []);
    backend.respond("generateHandoff", {
      path: ".foundry/prompts/handoff.md",
      warnings: [],
      bakeryNoteIssues: [{ entryId: "b-2", file: "02-panels.png", missing: "both" }],
    });

    const { user } = renderWithApp(
      <SidebarProvider>
        <Prompts projectId={PROJECT_ID} />
        <ContextualPanelFrame />
      </SidebarProvider>,
      { route: `/projects/${PROJECT_ID}/prompts` },
    );
    // The prompts route opens on Roadmap.
    await screen.findByText("No roadmap yet.");
    await user.click(await screen.findByRole("button", { name: "Generate hand-off" }));
    await user.click(await screen.findByRole("button", { name: "Edit 02-panels.png (no Take or Skip)" }));

    const sheet = await screen.findByRole("dialog");
    expect(within(sheet).getByText("02-panels.png")).toBeInTheDocument();
    expect(screen.queryByText("No roadmap yet.")).not.toBeInTheDocument();
  });

  it("shows no Bakery warning when every note is complete", async () => {
    backend.respond("listPrompts", []);
    backend.respond("getPlan", { revision: 1, slices: [], requirements: [], deferred: [], openQuestions: [], unresolvedConflicts: 0 });
    backend.respond("getTeam", null);
    backend.respond("generateHandoff", { path: ".foundry/prompts/handoff.md", warnings: [], bakeryNoteIssues: [] });

    const { user } = renderWithApp(<Prompts projectId={PROJECT_ID} />);
    await user.click(await screen.findByRole("button", { name: "Generate hand-off" }));
    await screen.findByText(".foundry/prompts/handoff.md");
    expect(screen.queryByRole("list", { name: "References with incomplete notes" })).not.toBeInTheDocument();
  });
});
