import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NvidiaPage } from "./NvidiaPage";
import { api, type NvidiaStatus } from "@/lib/tauri";

const status: NvidiaStatus = {
  info: { gpu: "NVIDIA GeForce RTX 4070", driver: "560.94" },
  games: [{ name: "Minecraft: Java Edition", exe: "javaw.exe", reflex: false, applied: false, fpsCap: null }],
  hasBackup: false,
};

beforeEach(() => {
  vi.spyOn(api, "tweaksOverview").mockResolvedValue({ tweaks: [], score: { value: null, counted: 0, applied: 0 } });
});
afterEach(() => vi.restoreAllMocks());

describe("NvidiaPage", () => {
  it("explains when there's no NVIDIA GPU", async () => {
    vi.spyOn(api, "nvidiaStatus").mockResolvedValue({ info: null, games: [], hasBackup: false });
    render(<NvidiaPage />);
    expect(await screen.findByText("No NVIDIA GPU found")).toBeInTheDocument();
  });

  it("applies a game profile and sets a frame cap", async () => {
    vi.spyOn(api, "nvidiaStatus").mockResolvedValue(status);
    const setGame = vi.spyOn(api, "nvidiaSetGame").mockResolvedValue(status);
    const setFps = vi.spyOn(api, "nvidiaSetFps").mockResolvedValue(status);
    render(<NvidiaPage />);
    await userEvent.click(await screen.findByRole("switch", { name: "Low-latency profile for Minecraft: Java Edition" }));
    expect(setGame).toHaveBeenCalledWith("javaw.exe", true);

    await userEvent.type(screen.getByLabelText("Frame cap"), "141");
    await userEvent.click(screen.getByRole("button", { name: "Set" }));
    expect(setFps).toHaveBeenCalledWith("javaw.exe", 141);
    expect(screen.getByRole("button", { name: "Restore all NVIDIA settings from backup" })).toBeDisabled();
  });
});
