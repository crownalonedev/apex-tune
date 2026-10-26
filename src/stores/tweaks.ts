import { create } from "zustand";
import { api, toAppError, type AppError, type Overview } from "@/lib/tauri";

interface TweaksState {
  overview: Overview | null;
  error: AppError | null;
  load: () => Promise<void>;
}

export const useTweaksStore = create<TweaksState>((set) => ({
  overview: null,
  error: null,
  load: async () => {
    try {
      set({ overview: await api.tweaksOverview(), error: null });
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },
}));
