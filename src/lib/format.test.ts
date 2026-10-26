import { describe, expect, it } from "vitest";
import { formatBytes, formatDriveSize, formatWmiDate, parseWmiDate, plural } from "./format";

describe("parseWmiDate", () => {
  it("applies the UTC offset in minutes", () => {
    expect(parseWmiDate("20261026120000.000000+060")?.toISOString()).toBe("2026-10-26T11:00:00.000Z");
    expect(parseWmiDate("20261026120000.000000-000")?.toISOString()).toBe("2026-10-26T12:00:00.000Z");
  });

  it("returns null for anything else", () => {
    expect(parseWmiDate("yesterday")).toBeNull();
    expect(formatWmiDate("yesterday")).toBe("yesterday");
  });
});

describe("plural", () => {
  it("picks the right form", () => {
    expect(plural(1, "change")).toBe("1 change");
    expect(plural(3, "change")).toBe("3 changes");
  });
});

describe("sizes", () => {
  it("formats memory in binary units", () => {
    expect(formatBytes(16 * 1024 ** 3)).toBe("16 GB");
    expect(formatBytes(11.5 * 1024 ** 3)).toBe("11.5 GB");
    expect(formatBytes(512)).toBe("512 B");
  });

  it("formats drives in decimal units", () => {
    expect(formatDriveSize(1_000_204_886_016)).toBe("1 TB");
    expect(formatDriveSize(512_110_190_592)).toBe("512 GB");
  });
});
