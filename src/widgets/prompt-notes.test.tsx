import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { PromptFile } from "@/lib/bindings";
import { backend } from "@/test/fake-backend";
import { PROJECT_ID, plan, slice } from "@/test/fixtures";
import { renderWithApp } from "@/test/render";
import { Prompts } from "./prompts";

const TASK = ".foundry/prompts/s-feed-ada.md";
const HANDOFF = ".foundry/prompts/handoff.md";
const GENERATED = "# Build the feed\n\nDo the work.";

const files: PromptFile[] = [
  { path: HANDOFF, kind: "handoff", sliceId: null, agentId: null, size: 900, modifiedMs: 0 },
  { path: TASK, kind: "task", sliceId: "s-feed", agentId: "ada", size: 1200, modifiedMs: 0 },
];

/** The Prompts page with one task prompt whose stored note is `note`; `setPromptNote` keeps it. */
function renderPage(note: string | null = null) {
  let stored = note;
  backend.respond("listPrompts", files);
  backend.respond("getPlan", plan({ slices: [slice({ id: "s-feed", name: "Feed" })] }));
  backend.respond("getTeam", null);
  backend.respond("getPrompt", (_projectId, file) => ({ text: file === TASK ? GENERATED : "Hand-off.", note: file === TASK ? stored : null }));
  backend.respond("setPromptNote", (_projectId, _file, next) => {
    stored = next.trim() === "" ? null : next;
    return null;
  });
  return renderWithApp(<Prompts projectId={PROJECT_ID} />);
}

async function openNote(user: ReturnType<typeof renderPage>["user"], trigger = "Add a note") {
  await user.click(await screen.findByRole("button", { name: trigger }));
  const box = await screen.findByRole("textbox", { name: "Note" });
  await waitFor(() => expect(box).toBeEnabled());
  return box;
}

describe("Notes on prompts", () => {
  it("offers a note under task prompts only", async () => {
    renderPage();
    expect(await screen.findAllByRole("button", { name: "Add a note" })).toHaveLength(1);
  });

  it("saves on blur and shows the Note added chip", async () => {
    const { user } = renderPage();
    const box = await openNote(user);
    expect(screen.queryByText("Note added")).not.toBeInTheDocument();

    await user.type(box, "Use the new tokens.");
    await user.tab();

    await waitFor(() => expect(backend.calls("setPromptNote")).toEqual([[PROJECT_ID, TASK, "Use the new tokens."]]));
    expect(await screen.findByText("Note added")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Edit note" })).toBeInTheDocument();
    expect(box).toHaveAccessibleDescription(/Your note is added to the end of the prompt when the agent starts\./);
  });

  it("saves with the Save button, once", async () => {
    const { user } = renderPage();
    const box = await openNote(user);
    await user.type(box, "Mind the empty state.");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(backend.calls("setPromptNote")).toEqual([[PROJECT_ID, TASK, "Mind the empty state."]]));
    await user.tab();
    expect(backend.calls("setPromptNote")).toHaveLength(1);
  });

  it("clearing the box removes the note", async () => {
    const { user } = renderPage("Old note");
    expect(await screen.findByText("Note added")).toBeInTheDocument();
    const box = await openNote(user, "Edit note");
    expect(box).toHaveValue("Old note");

    await user.clear(box);
    await user.tab();

    await waitFor(() => expect(backend.calls("setPromptNote")).toEqual([[PROJECT_ID, TASK, ""]]));
    await waitFor(() => expect(screen.queryByText("Note added")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Add a note" })).toBeInTheDocument();
  });

  it("counts characters and refuses to save over 8,000", async () => {
    const { user } = renderPage();
    const box = await openNote(user);
    expect(screen.getByText("0 / 8,000")).toBeInTheDocument();

    await user.type(box, "Hi 👋");
    // The backend counts characters, so the emoji is one.
    expect(screen.getByText("4 / 8,000")).toBeInTheDocument();

    fireEvent.change(box, { target: { value: "x".repeat(8001) } });
    expect(screen.getByText("8,001 / 8,000")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("1 character over the limit");
    expect(box).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    fireEvent.blur(box);
    expect(backend.calls("setPromptNote")).toHaveLength(0);

    fireEvent.change(box, { target: { value: "x".repeat(8000) } });
    expect(screen.getByText("8,000 / 8,000")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows the backend's refusal as-is", async () => {
    const { user } = renderPage();
    backend.failOnce("setPromptNote", { code: "invalid_input", message: "the note has control characters" });
    const box = await openNote(user);
    await user.type(box, "Hello");
    await user.tab();
    expect(await screen.findByText("the note has control characters")).toBeInTheDocument();
    expect(screen.queryByText("Note added")).not.toBeInTheDocument();
  });

  it("previews the generated text, then the note under its heading, and copies both", async () => {
    const { user } = renderPage("Use the new tokens.");
    await user.click(await screen.findByRole("button", { name: /^Feed/ }));
    const sheet = await screen.findByRole("dialog");
    const preview = await within(sheet).findByLabelText("Prompt text");
    const expected = `${GENERATED}\n\n## Notes from the user\n\nUse the new tokens.`;
    expect(preview.textContent).toBe(expected);

    await user.click(within(sheet).getByRole("button", { name: "Copy" }));
    expect(await navigator.clipboard.readText()).toBe(expected);
  });

  it("previews and copies the generated text alone without a note", async () => {
    const { user } = renderPage();
    await user.click(await screen.findByRole("button", { name: /^Feed/ }));
    const sheet = await screen.findByRole("dialog");
    const preview = await within(sheet).findByLabelText("Prompt text");
    expect(preview.textContent).toBe(GENERATED);

    await user.click(within(sheet).getByRole("button", { name: "Copy" }));
    expect(await navigator.clipboard.readText()).toBe(GENERATED);
  });
});
