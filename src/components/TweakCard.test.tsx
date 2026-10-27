import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TweakCard } from "./TweakCard";
import { api, type TweakInfo } from "@/lib/tauri";
import { useTweaksStore } from "@/stores/tweaks";

const tweak = (over: Partial<TweakInfo> = {}): TweakInfo => ({
  id: "hags",
  name: "Hardware-accelerated GPU scheduling",
  description: "Lets the GPU manage its own memory.",
  category: "gaming",
  risk: "safe",
  restart: "reboot",
  recommended: false,
  state: { state: "notApplied" },
  revertible: false,
  ...over,
});

let apply: ReturnType<typeof vi.fn<(id: string) => Promise<boolean>>>;
let revert: ReturnType<typeof vi.fn<(id: string) => Promise<boolean>>>;

beforeEach(() => {
  apply = vi.fn<(id: string) => Promise<boolean>>().mockResolvedValue(true);
  revert = vi.fn<(id: string) => Promise<boolean>>().mockResolvedValue(true);
  useTweaksStore.setState({ apply, revert, busy: new Set() });
});

afterEach(() => vi.restoreAllMocks());

describe("TweakCard", () => {
  it("applies Safe tweaks straight away", async () => {
    render(<TweakCard tweak={tweak()} />);
    await userEvent.click(screen.getByRole("switch"));
    expect(apply).toHaveBeenCalledWith("hags");
  });

  it("confirms Advanced tweaks and lists their changes first", async () => {
    vi.spyOn(api, "tweakPlan").mockResolvedValue([
      { target: "HKLM\\SYSTEM\\X\\Y", from: "(not set)", to: "1 (0x1)" },
    ]);
    render(<TweakCard tweak={tweak({ risk: "advanced" })} />);
    await userEvent.click(screen.getByRole("switch"));
    expect(await screen.findByRole("alertdialog")).toBeInTheDocument();
    expect(screen.getByText("HKLM\\SYSTEM\\X\\Y")).toBeInTheDocument();
    expect(apply).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Turn on" }));
    expect(apply).toHaveBeenCalledWith("hags");
  });

  it("reverts when switched off", async () => {
    render(<TweakCard tweak={tweak({ state: { state: "applied" }, revertible: true })} />);
    await userEvent.click(screen.getByRole("switch"));
    expect(revert).toHaveBeenCalledWith("hags");
  });

  it("can't switch off a setting Apex Tune didn't make", () => {
    render(<TweakCard tweak={tweak({ state: { state: "applied" }, revertible: false })} />);
    expect(screen.getByRole("switch")).toBeDisabled();
    expect(screen.getByText(/nothing to revert/)).toBeInTheDocument();
  });

  it("explains when a tweak doesn't apply", () => {
    render(<TweakCard tweak={tweak({ state: { state: "notApplicable", reason: "Needs Windows 11." } })} />);
    expect(screen.getByRole("switch")).toBeDisabled();
    expect(screen.getByText("Needs Windows 11.")).toBeInTheDocument();
  });
});
