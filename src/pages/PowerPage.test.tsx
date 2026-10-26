import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PowerPage } from "./PowerPage";
import { api, BALANCED_PLAN, type PowerOverview, type TweakInfo } from "@/lib/tauri";
import { usePowerStore } from "@/stores/power";

const APEX = "c855ae8b-fff9-45c7-8c29-99f6f893b54a";

const base: PowerOverview = {
  plans: [{ guid: BALANCED_PLAN, name: "Balanced", active: true, builtIn: true, apex: false }],
  laptop: false,
  balancedInstalled: true,
  settings: [{ label: "Minimum processor state", targetAc: "100%", targetDc: null, currentAc: null, currentDc: null }],
};

const tweak = (revertible: boolean): TweakInfo => ({
  id: "power_plan",
  name: "Apex Tune Gaming power plan",
  description: "",
  category: "power",
  risk: "safe",
  restart: "none",
  recommended: true,
  state: { state: "applied" },
  revertible,
});

beforeEach(() => {
  usePowerStore.setState({ overview: null, error: null, busy: null });
  vi.spyOn(api, "safetyStatus").mockResolvedValue({
    protectionEnabled: true,
    restorePoint: null,
    restorePointSkipped: false,
    activeChanges: 0,
    unreadableBackups: [],
  });
});

afterEach(() => vi.restoreAllMocks());

describe("PowerPage", () => {
  it("installs the gaming plan", async () => {
    const installed: PowerOverview = {
      ...base,
      plans: [
        { ...base.plans[0]!, active: false },
        { guid: APEX, name: "Apex Tune Gaming", active: true, builtIn: false, apex: true },
      ],
      settings: [{ ...base.settings[0]!, currentAc: "100%" }],
    };
    const overview = vi.spyOn(api, "powerOverview").mockResolvedValueOnce(base).mockResolvedValue(installed);
    vi.spyOn(api, "tweaksOverview").mockResolvedValue({ tweaks: [tweak(true)], score: { value: 100, counted: 1, applied: 1 } });
    const apply = vi.spyOn(api, "applyTweak").mockResolvedValue("applied");

    render(<PowerPage />);
    expect(await screen.findByText("Not installed.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Install and switch to it" }));

    expect(apply).toHaveBeenCalledWith("power_plan");
    expect(await screen.findByText("Installed and active.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove and switch back" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Switch" })).toBeInTheDocument();
    expect(overview).toHaveBeenCalledTimes(2);
  });

  it("explains how to get Balanced back when it was deleted", async () => {
    vi.spyOn(api, "powerOverview").mockResolvedValue({
      ...base,
      balancedInstalled: false,
      plans: [{ guid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", name: "High performance", active: true, builtIn: true, apex: false }],
    });
    vi.spyOn(api, "tweaksOverview").mockResolvedValue({ tweaks: [], score: { value: null, counted: 0, applied: 0 } });
    render(<PowerPage />);
    expect(await screen.findByText("powercfg -restoredefaultschemes")).toBeInTheDocument();
  });

  it("shows when the plan's value drifted from the target", async () => {
    vi.spyOn(api, "powerOverview").mockResolvedValue({
      ...base,
      plans: [{ guid: APEX, name: "Apex Tune Gaming", active: true, builtIn: false, apex: true }],
      settings: [{ ...base.settings[0]!, currentAc: "5%" }],
    });
    vi.spyOn(api, "tweaksOverview").mockResolvedValue({ tweaks: [tweak(false)], score: { value: 50, counted: 1, applied: 0 } });
    render(<PowerPage />);
    expect(await screen.findByText("Now 5%")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remove and switch back" })).not.toBeInTheDocument();
  });
});
