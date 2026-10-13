import { describe, expect, it } from "vitest";
import { hostOf, relativeTime, roleLabel, statusLabel } from "./format";

describe("format helpers", () => {
  it("labels WordPress statuses", () => {
    expect(statusLabel("publish")).toBe("Published");
    expect(statusLabel("future")).toBe("Scheduled");
    expect(statusLabel("custom")).toBe("Custom");
    expect(statusLabel(null)).toBe("");
  });

  it("formats relative times", () => {
    const now = new Date("2026-10-13T12:00:00");
    expect(relativeTime("2026-10-13T11:58:00", now)).toMatch(/2 minutes ago/);
    expect(relativeTime("not a date", now)).toBe("");
    expect(relativeTime(null, now)).toBe("");
  });

  it("labels roles", () => {
    expect(roleLabel(["editor"])).toBe("Editor");
    expect(roleLabel(["shop_manager"])).toBe("shop manager");
  });

  it("extracts hosts", () => {
    expect(hostOf("https://example.com/blog/")).toBe("example.com");
  });
});
