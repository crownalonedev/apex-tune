import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SafetyPanel } from "./SafetyPanel";
import { useSafetyStore } from "@/stores/safety";
import type { SafetyStatus } from "@/lib/tauri";

const base: SafetyStatus = {
  protectionEnabled: true,
  restorePoint: null,
  restorePointSkipped: false,
  activeChanges: 0,
  unreadableBackups: [],
};

function setStatus(status: Partial<SafetyStatus>) {
  // Keep `load` from replacing the seeded status with preview data.
  useSafetyStore.setState({ status: { ...base, ...status }, error: null, busy: null, load: async () => {} });
}

describe("SafetyPanel", () => {
  beforeEach(() => setStatus({}));

  it("explains that a restore point is made automatically", () => {
    render(<SafetyPanel />);
    expect(screen.getByText(/made automatically before your first change/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Revert everything" })).toBeDisabled();
  });

  it("offers to turn on System Protection when it's off", () => {
    setStatus({ protectionEnabled: false });
    render(<SafetyPanel />);
    expect(screen.getByRole("button", { name: "Turn on System Protection" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue without" })).toBeInTheDocument();
  });

  it("asks for confirmation before reverting everything", async () => {
    const revertEverything = vi.fn().mockResolvedValue({ reverted: ["Game Mode"], failed: [] });
    setStatus({ activeChanges: 2 });
    useSafetyStore.setState({ revertEverything });
    render(<SafetyPanel />);

    expect(screen.getByText("2 changes can be reverted.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Revert everything" }));
    expect(screen.getByRole("alertdialog", { name: "Revert 2 changes?" })).toBeInTheDocument();
    expect(revertEverything).not.toHaveBeenCalled();

    await userEvent.click(screen.getAllByRole("button", { name: "Revert everything" }).at(-1)!);
    expect(revertEverything).toHaveBeenCalledOnce();
  });

  it("shows the restore point time", () => {
    setStatus({ restorePoint: { sequence: 4, description: "Apex Tune: before changes", created: "20261026120000.000000-000" } });
    render(<SafetyPanel />);
    expect(screen.getByText(/^Created .* for this session\.$/)).toBeInTheDocument();
  });
});
