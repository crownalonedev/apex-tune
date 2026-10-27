import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type ProfileView, type SessionView } from "@/lib/tauri";
import { useSafetyStore } from "@/stores/safety";
import { HistoryPanel } from "./profiles/HistoryPanel";
import { ProfilesPanel } from "./profiles/ProfilesPanel";

const profile: ProfileView = {
  format: "apex-tune-profile",
  version: 1,
  id: "builtin.competitive",
  name: "Competitive",
  description: "",
  createdAt: "2026-10-26T12:00:00Z",
  tweaks: ["a", "b", "c"],
  builtIn: true,
  applied: 1,
};

const session: SessionView = {
  id: "s1",
  startedAt: "2026-10-26T12:00:00Z",
  restorePoint: null,
  restorePointSkipped: true,
  entries: [
    {
      session: "s1",
      entry: "e1",
      name: "Game Mode",
      action: "apply",
      status: "done",
      at: "2026-10-26T12:01:00Z",
      error: null,
      revertible: true,
      reverted: false,
      changes: [{ target: "HKCU\\Software\\Microsoft\\GameBar\\AutoGameModeEnabled", before: "(not set)", after: "1 (0x1)" }],
    },
  ],
};

beforeEach(() => {
  vi.spyOn(api, "safetyStatus").mockResolvedValue({ protectionEnabled: true, restorePoint: null, restorePointSkipped: false, activeChanges: 1, unreadableBackups: [] });
  vi.spyOn(api, "tweaksOverview").mockResolvedValue({ tweaks: [], score: { value: null, counted: 0, applied: 0 } });
  useSafetyStore.setState({ status: null });
});
afterEach(() => vi.restoreAllMocks());

describe("Profiles", () => {
  it("applies a profile, optionally switching off everything else", async () => {
    vi.spyOn(api, "profilesList").mockResolvedValue([profile]);
    const apply = vi.spyOn(api, "profileApply").mockResolvedValue({ results: [], restart: "none" });
    render(<ProfilesPanel />);
    expect(await screen.findByText("1 of 3 tweaks on")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    const dialog = await screen.findByRole("alertdialog");
    await userEvent.click(within(dialog).getByRole("checkbox"));
    await userEvent.click(within(dialog).getByRole("button", { name: "Apply" }));
    expect(apply).toHaveBeenCalledWith("builtin.competitive", true);
    expect(screen.queryByRole("button", { name: "Delete Competitive" })).not.toBeInTheDocument();
  });
});

describe("What changed", () => {
  it("shows before and after values and reverts one change", async () => {
    vi.spyOn(api, "backupsSessions").mockResolvedValue([session]);
    const revert = vi.spyOn(api, "backupsRevertEntry").mockResolvedValue();
    render(<HistoryPanel />);
    await userEvent.click(await screen.findByRole("button", { name: /Game Mode/ }));
    expect(screen.getByText("(not set)")).toBeInTheDocument();
    expect(screen.getByText("1 (0x1)")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Revert" }));
    expect(revert).toHaveBeenCalledWith("s1", "e1");
  });
});
