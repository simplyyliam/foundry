import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { backend } from "./fake-backend";

// Every test talks to the fake backend instead of Tauri. `bindings.ts` exports only these
// two values (the rest are types), so nothing else needs replacing.
vi.mock("@/lib/bindings", async () => {
  const { backend } = await import("./fake-backend");
  return { commands: backend.commands, events: backend.events };
});

afterEach(() => {
  cleanup();
  backend.reset();
});

// jsdom lacks these browser APIs; Base UI, motion and React Flow call them.
if (!window.matchMedia) {
  window.matchMedia = (query: string): MediaQueryList => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  });
}

class NoopObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}
globalThis.ResizeObserver ??= NoopObserver as unknown as typeof ResizeObserver;
globalThis.IntersectionObserver ??= NoopObserver as unknown as typeof IntersectionObserver;

Element.prototype.scrollIntoView ??= function scrollIntoView() {};
Element.prototype.hasPointerCapture ??= () => false;
Element.prototype.releasePointerCapture ??= () => {};
