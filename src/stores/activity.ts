import { create } from "zustand";
import { api, onActivity, type ActivityEntry } from "@/lib/tauri";

const MAX_ENTRIES = 500;

interface ActivityState {
  entries: ActivityEntry[];
  open: boolean;
  /** Errors and warnings that arrived while the log was closed. */
  unseenProblems: number;
  setOpen: (open: boolean) => void;
  append: (entry: ActivityEntry) => void;
  merge: (entries: ActivityEntry[]) => void;
  /** Loads history and subscribes to new entries. Returns an unsubscribe. */
  connect: () => Promise<() => void>;
}

/** Merges by id (backend ids are unique and increasing), keeping the newest MAX_ENTRIES. */
export function mergeEntries(current: ActivityEntry[], incoming: ActivityEntry[]): ActivityEntry[] {
  const byId = new Map<number, ActivityEntry>();
  for (const entry of current) byId.set(entry.id, entry);
  for (const entry of incoming) byId.set(entry.id, entry);
  return [...byId.values()].sort((a, b) => a.id - b.id).slice(-MAX_ENTRIES);
}

export const useActivityStore = create<ActivityState>((set, get) => ({
  entries: [],
  open: false,
  unseenProblems: 0,
  setOpen: (open) => set(open ? { open, unseenProblems: 0 } : { open }),
  append: (entry) =>
    set((state) => ({
      entries: mergeEntries(state.entries, [entry]),
      unseenProblems:
        !state.open && entry.level !== "info" ? state.unseenProblems + 1 : state.unseenProblems,
    })),
  merge: (entries) => set((state) => ({ entries: mergeEntries(state.entries, entries) })),
  connect: async () => {
    // Subscribe first so nothing logged between the two calls is missed.
    const unlisten = await onActivity((entry) => get().append(entry));
    get().merge(await api.activityRecent());
    return unlisten;
  },
}));
