import { create } from "zustand";
import { APEX_PLAN_TWEAK, api, toAppError, type AppError, type PowerOverview } from "@/lib/tauri";
import { useSafetyStore } from "@/stores/safety";
import { useTweaksStore } from "@/stores/tweaks";

type Busy = "install" | "remove" | { switching: string } | null;

interface PowerState {
  overview: PowerOverview | null;
  error: AppError | null;
  busy: Busy;
  load: () => Promise<void>;
  install: () => Promise<boolean>;
  remove: () => Promise<boolean>;
  switchTo: (guid: string) => Promise<boolean>;
}

export const usePowerStore = create<PowerState>((set, get) => {
  /** Runs a change, then refreshes everything that depends on it. */
  const run = async (busy: Busy, action: () => Promise<unknown>): Promise<boolean> => {
    set({ busy, error: null });
    try {
      await action();
      return true;
    } catch (error) {
      set({ error: toAppError(error) });
      return false;
    } finally {
      set({ busy: null });
      await Promise.all([get().load(), useTweaksStore.getState().load(), useSafetyStore.getState().load()]);
    }
  };

  return {
    overview: null,
    error: null,
    busy: null,
    load: async () => {
      try {
        set({ overview: await api.powerOverview() });
      } catch (error) {
        set({ error: toAppError(error) });
      }
    },
    install: () => run("install", () => api.applyTweak(APEX_PLAN_TWEAK)),
    remove: () => run("remove", () => api.revertTweak(APEX_PLAN_TWEAK)),
    switchTo: (guid) => run({ switching: guid }, () => api.powerSwitch(guid)),
  };
});
