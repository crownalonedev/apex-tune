import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppsPanel } from "./debloat/AppsPanel";
import { BackgroundPanel } from "./debloat/BackgroundPanel";
import { StartupPanel } from "./debloat/StartupPanel";
import { api, type AppRow } from "@/lib/tauri";

const app = (name: string, group: AppRow["group"], over: Partial<AppRow> = {}): AppRow => ({
  name,
  displayName: name.split(".").pop()!,
  group,
  note: `${name} note`,
  installed: true,
  restorable: false,
  ...over,
});

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

describe("Debloat apps", () => {
  it("removes only the selected apps after confirming, and never offers kept ones", async () => {
    vi.spyOn(api, "debloatApps").mockResolvedValue([
      app("Clipchamp.Clipchamp", "removable"),
      app("Microsoft.BingNews", "removable"),
      app("Microsoft.GamingServices", "keep"),
      app("Microsoft.Old", "removable", { installed: false, restorable: true }),
    ]);
    const remove = vi.spyOn(api, "debloatRemoveApps").mockResolvedValue([{ id: "Clipchamp.Clipchamp", ok: true, error: null }]);

    render(<AppsPanel />);
    await userEvent.click(await screen.findByLabelText(/Clipchamp/));
    expect(screen.queryByLabelText(/GamingServices/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Bring back" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Remove 1 app" }));
    const dialog = await screen.findByRole("alertdialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    expect(remove).toHaveBeenCalledWith(["Clipchamp.Clipchamp"]);
  });
});

describe("Debloat startup", () => {
  it("disables protected entries' switches", async () => {
    vi.spyOn(api, "debloatStartup").mockResolvedValue([
      { id: "userRun:Discord", name: "Discord", command: "d.exe", source: "userRun", enabled: true, protected: null },
      { id: "machineRun:Vanguard", name: "Riot Vanguard", command: "vgtray.exe", source: "machineRun", enabled: true, protected: "Anti-cheat; games need it running." },
    ]);
    const set = vi.spyOn(api, "debloatSetStartup").mockResolvedValue();
    render(<StartupPanel />);
    expect(await screen.findByRole("switch", { name: "Riot Vanguard starts with Windows" })).toBeDisabled();
    await userEvent.click(screen.getByRole("switch", { name: "Discord starts with Windows" }));
    expect(set).toHaveBeenCalledWith("userRun:Discord", false);
  });
});

describe("Debloat background", () => {
  it("pre-selects safe items but not Moderate services", async () => {
    vi.spyOn(api, "debloatBackground").mockResolvedValue({
      tasks: [{ path: "\\T\\A", name: "A", description: "", enabled: true }],
      services: [
        { name: "DiagTrack", display: "Telemetry", description: "", current: "automatic", running: true, recommended: "disabled", risk: "moderate" },
        { name: "Fax", display: "Fax", description: "", current: "manual", running: false, recommended: "disabled", risk: "safe" },
      ],
      changed: [],
    });
    const apply = vi.spyOn(api, "debloatApplyBackground").mockResolvedValue([]);
    render(<BackgroundPanel />);
    await userEvent.click(await screen.findByRole("button", { name: "Apply 2 changes" }));
    expect(apply).toHaveBeenCalledWith(["\\T\\A"], ["Fax"]);
  });
});
