import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SystemCheck } from "@/components/dashboard/SystemCheck";
import { api } from "@/lib/tauri";
import { GamesPanel } from "./tweaks/GamesPanel";
import { StoragePanel } from "./tweaks/StoragePanel";

beforeEach(() => {
  vi.spyOn(api, "safetyStatus").mockResolvedValue({
    protectionEnabled: true,
    restorePoint: null,
    restorePointSkipped: false,
    activeChanges: 0,
    unreadableBackups: [],
  });
});
afterEach(() => vi.restoreAllMocks());

describe("Games", () => {
  it("confirms before raising a game's CPU priority", async () => {
    vi.spyOn(api, "gamesList").mockResolvedValue([
      { name: "VALORANT", exe: "VALORANT-Win64-Shipping.exe", path: null, source: "known", note: null, highPriority: false, fsoDisabled: false },
    ]);
    const set = vi.spyOn(api, "gameSetPriority").mockResolvedValue();
    render(<GamesPanel />);
    await userEvent.click(await screen.findByRole("switch", { name: "High CPU priority for VALORANT" }));
    expect(set).not.toHaveBeenCalled();
    await userEvent.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Turn on" }));
    expect(set).toHaveBeenCalledWith("VALORANT-Win64-Shipping.exe", true);
    // No path known, so the fullscreen-optimizations switch is unavailable.
    expect(screen.getByRole("switch", { name: "Fullscreen optimizations off for VALORANT" })).toBeDisabled();
  });
});

describe("Storage", () => {
  it("asks before deleting and reports what was freed", async () => {
    vi.spyOn(api, "storageScan").mockResolvedValue([
      { target: "userTemp", name: "Your temporary files", description: "", warning: null, bytes: 1024 ** 3, files: 10 },
    ]);
    const clean = vi.spyOn(api, "storageClean").mockResolvedValue({ freedBytes: 1024 ** 3, deletedFiles: 10, skippedFiles: 0 });
    render(<StoragePanel />);
    await userEvent.click(await screen.findByRole("button", { name: "Clean" }));
    const dialog = await screen.findByRole("alertdialog", { name: /Delete 1 GB/ });
    await userEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect(clean).toHaveBeenCalledWith("userTemp");
  });
});

describe("System check", () => {
  it("offers the refresh-rate fix and applies it", async () => {
    vi.spyOn(api, "insightsRun").mockResolvedValue([
      { id: "r", title: "XG27 runs at 60 Hz but supports 165 Hz", detail: "", level: "warn", fix: { kind: "refreshRate", device: "D1", hz: 165 } },
      { id: "s", title: "Secure Boot is on", detail: "", level: "good", fix: null },
    ]);
    const fix = vi.spyOn(api, "insightsFix").mockResolvedValue();
    render(<SystemCheck />);
    expect(await screen.findByText("1 to look at")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Switch to 165 Hz" }));
    expect(fix).toHaveBeenCalledWith({ kind: "refreshRate", device: "D1", hz: 165 });
  });
});
