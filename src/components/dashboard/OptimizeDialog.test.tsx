import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OptimizeDialog } from "./OptimizeDialog";
import { api, type PresetItem, type SafetyStatus } from "@/lib/tauri";
import { useAppStore } from "@/stores/app";

const item = (id: string, name: string, restart: PresetItem["restart"] = "none"): PresetItem => ({
  id,
  name,
  description: `${name} description`,
  category: "gaming",
  risk: "safe",
  restart,
  recommended: true,
  state: { state: "notApplied" },
  changes: [{ target: `HKCU\\Software\\${id}`, from: "(not set)", to: "0 (0x0)" }],
});

const status = {} as SafetyStatus;

beforeEach(() => {
  useAppStore.setState({ pendingRestart: "none" });
  vi.spyOn(api, "presetPreview").mockResolvedValue([item("game_mode", "Game Mode"), item("game_dvr_off", "Turn off background recording", "signOut")]);
  vi.spyOn(api, "tweaksOverview").mockResolvedValue({ tweaks: [], score: { value: null, counted: 0, applied: 0 } });
  vi.spyOn(api, "safetyStatus").mockResolvedValue(status);
});

afterEach(() => vi.restoreAllMocks());

describe("OptimizeDialog", () => {
  it("previews changes and applies only the selected tweaks", async () => {
    const user = userEvent.setup();
    vi.spyOn(api, "createRestorePoint").mockResolvedValue(status);
    const applyPreset = vi.spyOn(api, "applyPreset").mockResolvedValue({
      results: [{ id: "game_dvr_off", name: "Turn off background recording", status: "applied", errorKind: null, error: null }],
      restart: "signOut",
    });

    render(<OptimizeDialog />);
    await user.click(screen.getByRole("button", { name: "Optimize for Gaming" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("HKCU\\Software\\game_mode")).toBeInTheDocument();

    await user.click(within(dialog).getByLabelText("Game Mode"));
    await user.click(within(dialog).getByRole("button", { name: "Apply 1 tweak" }));

    expect(applyPreset).toHaveBeenCalledWith(["game_dvr_off"]);
    expect(await within(dialog).findByText(/1 tweak applied\. Sign out and back in to finish\./)).toBeInTheDocument();
    expect(useAppStore.getState().pendingRestart).toBe("signOut");
  });

  it("stops before changing anything when a restore point can't be made", async () => {
    const user = userEvent.setup();
    vi.spyOn(api, "createRestorePoint").mockRejectedValue({
      kind: "restorePointRequired",
      message: "System Protection is off for the system drive.",
    });
    const applyPreset = vi.spyOn(api, "applyPreset");

    render(<OptimizeDialog />);
    await user.click(screen.getByRole("button", { name: "Optimize for Gaming" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(await within(dialog).findByRole("button", { name: "Apply 2 tweaks" }));

    expect(await within(dialog).findByText("System Protection is off for the system drive.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Continue without a restore point" })).toBeInTheDocument();
    expect(applyPreset).not.toHaveBeenCalled();
  });
});
