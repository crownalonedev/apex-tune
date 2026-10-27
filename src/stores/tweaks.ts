import { create } from "zustand";
import { toast } from "sonner";
import { api, toAppError, type AppError, type Overview, type TweakInfo } from "@/lib/tauri";
import { useAppStore } from "@/stores/app";
import { useSafetyStore } from "@/stores/safety";

interface TweaksState {
  overview: Overview | null;
  error: AppError | null;
  /** Ids of tweaks with a change in flight. */
  busy: Set<string>;
  load: () => Promise<void>;
  byId: (id: string) => TweakInfo | undefined;
  apply: (id: string) => Promise<boolean>;
  revert: (id: string) => Promise<boolean>;
}

export const useTweaksStore = create<TweaksState>((set, get) => {
  const run = async (id: string, verb: "apply" | "revert"): Promise<boolean> => {
    const tweak = get().byId(id);
    set((s) => ({ busy: new Set(s.busy).add(id) }));
    try {
      if (verb === "apply") {
        const outcome = await api.applyTweak(id);
        if (outcome === "applied" && tweak) useAppStore.getState().needRestart(tweak.restart);
        toast.success(`${tweak?.name ?? "Tweak"} applied`);
      } else {
        await api.revertTweak(id);
        if (tweak) useAppStore.getState().needRestart(tweak.restart);
        toast.success(`${tweak?.name ?? "Tweak"} reverted`);
      }
      return true;
    } catch (error) {
      const e = toAppError(error);
      toast.error(e.message, { duration: 12_000 });
      return false;
    } finally {
      set((s) => {
        const busy = new Set(s.busy);
        busy.delete(id);
        return { busy };
      });
      await Promise.all([get().load(), useSafetyStore.getState().load()]);
    }
  };

  return {
    overview: null,
    error: null,
    busy: new Set(),
    load: async () => {
      try {
        set({ overview: await api.tweaksOverview(), error: null });
      } catch (error) {
        set({ error: toAppError(error) });
      }
    },
    byId: (id) => get().overview?.tweaks.find((t) => t.id === id),
    apply: (id) => run(id, "apply"),
    revert: (id) => run(id, "revert"),
  };
});
