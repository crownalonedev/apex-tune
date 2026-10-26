import { beforeEach, describe, expect, it } from "vitest";
import { mergeEntries, useActivityStore } from "./activity";
import type { ActivityEntry } from "@/lib/tauri";

const entry = (id: number, level: ActivityEntry["level"] = "info"): ActivityEntry => ({
  id,
  at: "2026-10-26T12:00:00Z",
  level,
  message: `entry ${id}`,
  subject: null,
});

describe("mergeEntries", () => {
  it("dedupes by id and keeps id order", () => {
    const merged = mergeEntries([entry(1), entry(3)], [entry(2), entry(3)]);
    expect(merged.map((e) => e.id)).toEqual([1, 2, 3]);
  });

  it("keeps only the newest 500", () => {
    const many = Array.from({ length: 520 }, (_, i) => entry(i + 1));
    const merged = mergeEntries([], many);
    expect(merged).toHaveLength(500);
    expect(merged[0]?.id).toBe(21);
  });
});

describe("activity store", () => {
  beforeEach(() => useActivityStore.setState({ entries: [], open: false, unseenProblems: 0 }));

  it("counts problems that arrive while closed and clears them on open", () => {
    const { append, setOpen } = useActivityStore.getState();
    append(entry(1));
    append(entry(2, "error"));
    append(entry(3, "warn"));
    expect(useActivityStore.getState().unseenProblems).toBe(2);
    setOpen(true);
    expect(useActivityStore.getState().unseenProblems).toBe(0);
  });
});
