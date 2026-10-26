import { create } from "zustand";
import { api, toAppError, type AppError, type AppInfo } from "@/lib/tauri";
import type { PageId } from "@/lib/routes";

interface AppState {
  page: PageId;
  info: AppInfo | null;
  infoError: AppError | null;
  setPage: (page: PageId) => void;
  loadInfo: () => Promise<void>;
}

export const useAppStore = create<AppState>((set) => ({
  page: "dashboard",
  info: null,
  infoError: null,
  setPage: (page) => set({ page }),
  loadInfo: async () => {
    try {
      set({ info: await api.appInfo(), infoError: null });
    } catch (error) {
      set({ infoError: toAppError(error) });
    }
  },
}));
