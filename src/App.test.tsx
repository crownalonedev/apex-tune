import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import App from "./App";
import { useActivityStore } from "./stores/activity";
import { useAppStore } from "./stores/app";

beforeEach(() => {
  useAppStore.setState({ page: "dashboard", info: null, infoError: null });
  useActivityStore.setState({ entries: [], open: false, unseenProblems: 0 });
});

describe("App shell", () => {
  it("navigates between pages from the sidebar", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { level: 1, name: "Dashboard" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Power" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Power" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Power" })).toHaveAttribute("aria-current", "page");
  });

  it("shows preview system info and opens the activity log", async () => {
    render(<App />);
    expect(await screen.findByText(/Browser preview. Nothing here touches your system./)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Activity log" }));
    expect(screen.getByRole("region", { name: "Activity log" })).toBeInTheDocument();
  });
});
