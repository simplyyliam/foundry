import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { usePortalContainer } from "@/lib/portal-container";
import { WindowFrame } from "./WindowFrame";

function PortalProbe() {
  const container = usePortalContainer();
  return <span data-testid="probe">{container?.dataset.slot ?? "body"}</span>;
}

describe("WindowFrame", () => {
  it("fills the window with no margin, radius, shadow or resize handles", () => {
    const { container } = render(<WindowFrame titlebar={<div>titlebar</div>}>content</WindowFrame>);
    const card = container.firstElementChild as HTMLElement;

    expect(container.children).toHaveLength(1);
    expect(screen.getByText("titlebar")).toBeInTheDocument();
    expect(screen.getByText("content")).toBeInTheDocument();
    expect(card.getAttribute("style")).toBeNull();
    expect(card.className).not.toMatch(/rounded|shadow/);
    expect(card.className).toContain("p-(--shell-padding)");
    // Only the titlebar, the content area and the modal layer: no handle strips.
    expect(card.children).toHaveLength(3);
  });

  it("gives dialogs and sheets the modal layer to portal into", () => {
    render(
      <WindowFrame titlebar={<PortalProbe />}>
        <PortalProbe />
      </WindowFrame>,
    );

    expect(screen.getAllByTestId("probe").map((el) => el.textContent)).toEqual(["modal-layer", "modal-layer"]);
  });
});
