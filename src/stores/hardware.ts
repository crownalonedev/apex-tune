import { create } from "zustand";
import { api, toAppError, type AppError, type HardwareSummary } from "@/lib/tauri";

interface HardwareState {
  summary: HardwareSummary | null;
  error: AppError | null;
  load: (refresh?: boolean) => Promise<void>;
}

export const useHardwareStore = create<HardwareState>((set) => ({
  summary: null,
  error: null,
  load: async (refresh = false) => {
    try {
      set({ summary: await api.hardwareSummary(refresh), error: null });
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },
}));
