import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsPage } from "./SettingsPage";
import { api, type SettingsView } from "@/lib/tauri";
import { useSettingsStore } from "@/stores/settings";

const view: SettingsView = {
  theme: "graphite",
  motion: "system",
  minimizeToTray: false,
  checkForUpdates: true,
  startWithWindows: false,
  dataDir: "C:\\ProgramData\\ApexTune",
  version: "0.1.0",
};

beforeEach(() => {
  useSettingsStore.setState({ view: null });
  vi.spyOn(api, "settingsGet").mockResolvedValue(view);
});
afterEach(() => vi.restoreAllMocks());

describe("Settings", () => {
  it("switches theme and saves it", async () => {
    const save = vi.spyOn(api, "settingsSet").mockResolvedValue();
    render(<SettingsPage />);
    await userEvent.click(await screen.findByRole("radio", { name: /Ember/ }));
    expect(document.documentElement.dataset.theme).toBe("ember");
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ theme: "ember" }));
    expect(screen.getByRole("radio", { name: /Ember/ })).toHaveAttribute("aria-checked", "true");
  });

  it("reduced motion is applied to the document", async () => {
    vi.spyOn(api, "settingsSet").mockResolvedValue();
    render(<SettingsPage />);
    await userEvent.click(await screen.findByRole("radio", { name: "Reduced" }));
    expect(document.documentElement.dataset.motion).toBe("reduced");
  });

  it("turns on start with Windows", async () => {
    const autostart = vi.spyOn(api, "settingsSetAutostart").mockResolvedValue();
    render(<SettingsPage />);
    await userEvent.click(await screen.findByRole("switch", { name: "Start Apex Tune when I sign in" }));
    expect(autostart).toHaveBeenCalledWith(true);
  });
});
