import { afterEach, describe, expect, it, vi } from "vitest";
import { checkForUpdate, isNewer } from "./updates";

afterEach(() => vi.restoreAllMocks());

describe("updates", () => {
  it("compares versions", () => {
    expect(isNewer("v0.2.0", "0.1.9")).toBe(true);
    expect(isNewer("0.1.0", "0.1.0")).toBe(false);
    expect(isNewer("1.0", "1.0.1")).toBe(false);
  });

  it("says when no update source is configured", async () => {
    expect(await checkForUpdate("0.1.0", undefined)).toEqual({ kind: "notConfigured" });
  });

  it("reports a newer release", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ tag_name: "v0.2.0", html_url: "https://example.test/r" }), { status: 200 }),
    );
    expect(await checkForUpdate("0.1.0", "https://example.test/latest")).toEqual({
      kind: "available",
      latest: "v0.2.0",
      url: "https://example.test/r",
    });
  });
});
