import { describe, expect, it } from "vitest";
import { api, toAppError } from "./tauri";

describe("toAppError", () => {
  it("passes backend errors through", () => {
    expect(toAppError({ kind: "windows", message: "Access denied" })).toEqual({
      kind: "windows",
      message: "Access denied",
    });
  });

  it("wraps other values", () => {
    expect(toAppError(new Error("boom"))).toEqual({ kind: "other", message: "boom" });
    expect(toAppError("plain")).toEqual({ kind: "other", message: "plain" });
  });
});

describe("api outside Tauri", () => {
  it("returns labelled preview data instead of calling the backend", async () => {
    const info = await api.appInfo();
    expect(info.elevated).toBe(false);
    expect(info.osName).toMatch(/preview/i);
    expect(await api.activityRecent()).toEqual([]);
  });
});
