import type { ReactElement } from "react";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";

/**
 * Renders `ui` inside what every page has around it (a router), with a user-event session.
 * The fake backend is already installed by `setup.ts`; register answers before rendering.
 */
export function renderWithApp(ui: ReactElement, { route = "/" }: { route?: string } = {}) {
  const user = userEvent.setup();
  return { user, ...render(<MemoryRouter initialEntries={[route]}>{ui}</MemoryRouter>) };
}
