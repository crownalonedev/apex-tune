import { create } from "zustand";
import { api, toAppError, type AppError, type RevertReport, type SafetyStatus } from "@/lib/tauri";

type Busy = "restorePoint" | "protection" | "skip" | "revertAll" | null;

interface SafetyState {
  status: SafetyStatus | null;
  error: AppError | null;
  busy: Busy;
  load: () => Promise<void>;
  createRestorePoint: () => Promise<void>;
  enableProtection: () => Promise<void>;
  skipRestorePoint: () => Promise<void>;
  /** Resolves with the report; rejects with an AppError. */
  revertEverything: () => Promise<RevertReport>;
}

export const useSafetyStore = create<SafetyState>((set, get) => {
  /** Runs an action that returns a fresh status, tracking busy and error. */
  const run = async (busy: Exclude<Busy, null>, action: () => Promise<SafetyStatus>) => {
    set({ busy, error: null });
    try {
      set({ status: await action() });
    } catch (error) {
      set({ error: toAppError(error) });
    } finally {
      set({ busy: null });
    }
  };

  return {
    status: null,
    error: null,
    busy: null,
    load: async () => {
      try {
        set({ status: await api.safetyStatus(), error: null });
      } catch (error) {
        set({ error: toAppError(error) });
      }
    },
    createRestorePoint: () => run("restorePoint", api.createRestorePoint),
    enableProtection: async () => {
      await run("protection", api.enableSystemProtection);
      // Turning protection on is only useful with a point made right after.
      if (!get().error) await run("restorePoint", api.createRestorePoint);
    },
    skipRestorePoint: () => run("skip", api.skipRestorePoint),
    revertEverything: async () => {
      set({ busy: "revertAll", error: null });
      try {
        return await api.revertEverything();
      } catch (error) {
        const appError = toAppError(error);
        set({ error: appError });
        throw appError;
      } finally {
        set({ busy: null });
        await get().load();
      }
    },
  };
});
