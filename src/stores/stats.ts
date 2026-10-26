import { create } from "zustand";
import { api, onStats, type StatsTick } from "@/lib/tauri";

/** Seconds of history kept for the sparklines. */
export const HISTORY = 60;

interface StatsState {
  history: StatsTick[];
  push: (tick: StatsTick) => void;
  /** Starts sampling while the caller is mounted. Returns a stop function. */
  watch: () => () => void;
}

export const useStatsStore = create<StatsState>((set, get) => ({
  history: [],
  push: (tick) => set((s) => ({ history: [...s.history, tick].slice(-HISTORY) })),
  watch: () => {
    let stopped = false;
    let unlisten: (() => void) | undefined;
    void onStats((tick) => get().push(tick)).then((fn) => {
      if (stopped) fn();
      else unlisten = fn;
    });
    void api.statsSubscribe();
    return () => {
      stopped = true;
      unlisten?.();
      void api.statsUnsubscribe();
    };
  },
}));
