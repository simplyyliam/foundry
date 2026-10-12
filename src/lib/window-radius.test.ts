import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { backend } from "@/test/fake-backend";
import { loadWindowRadius } from "./window-radius";

let root: HTMLElement;

beforeEach(() => {
  root = document.createElement("html");
});

afterEach(() => vi.restoreAllMocks());

const radiusVar = () => root.style.getPropertyValue("--window-radius");

describe("loadWindowRadius", () => {
  it("sets the variable and attribute when the backend returns a radius", async () => {
    backend.respond("getWindowRadius", { radius: 12 });
    await loadWindowRadius(root);

    expect(backend.calls("getWindowRadius")).toHaveLength(1);
    expect(radiusVar()).toBe("12px");
    expect(root.dataset.windowRadius).toBe("custom");
    expect(root.dataset.insetRadius).toBe("window");
  });

  it("keeps the default inset radii when the inner radius would be under 4px", async () => {
    backend.respond("getWindowRadius", { radius: 8 });
    await loadWindowRadius(root);

    expect(radiusVar()).toBe("8px");
    expect(root.dataset.windowRadius).toBe("custom");
    expect(root.dataset.insetRadius).toBeUndefined();
  });

  it("sets nothing when the radius is null", async () => {
    backend.respond("getWindowRadius", { radius: null });
    await loadWindowRadius(root);

    expect(radiusVar()).toBe("");
    expect(root.hasAttribute("data-window-radius")).toBe(false);
    expect(root.hasAttribute("data-inset-radius")).toBe(false);
  });

  it("sets nothing when the command fails", async () => {
    backend.fail("getWindowRadius", { code: "internal", message: "no window" });
    await loadWindowRadius(root);

    expect(radiusVar()).toBe("");
    expect(root.hasAttribute("data-window-radius")).toBe(false);
  });

  it("sets nothing when the IPC call throws", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    backend.throwOnce("getWindowRadius", new Error("IPC down"));
    await loadWindowRadius(root);

    expect(radiusVar()).toBe("");
    expect(root.hasAttribute("data-window-radius")).toBe(false);
  });
});
