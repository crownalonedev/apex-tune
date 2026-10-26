import { create } from "zustand";
import { api, toAppError, type AppError, type AppInfo, type Restart } from "@/lib/tauri";
import type { PageId } from "@/lib/routes";

interface AppState {
  page: PageId;
  info: AppInfo | null;
  infoError: AppError | null;
  /** The strongest restart needed by changes made since Apex Tune started. */
  pendingRestart: Restart;
  setPage: (page: PageId) => void;
  needRestart: (restart: Restart) => void;
  loadInfo: () => Promise<void>;
}

export const useAppStore = create<AppState>((set) => ({
  page: "dashboard",
  info: null,
  infoError: null,
  pendingRestart: "none",
  setPage: (page) => set({ page }),
  needRestart: (restart) =>
    set((s) => ({ pendingRestart: strongerRestart(s.pendingRestart, restart) })),
  loadInfo: async () => {
    try {
      set({ info: await api.appInfo(), infoError: null });
    } catch (error) {
      set({ infoError: toAppError(error) });
    }
  },
}));

const restartOrder: Restart[] = ["none", "explorer", "signOut", "reboot"];

export function strongerRestart(a: Restart, b: Restart): Restart {
  return restartOrder.indexOf(a) >= restartOrder.indexOf(b) ? a : b;
}
