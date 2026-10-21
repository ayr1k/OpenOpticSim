import { describe, expect, it } from "vitest";
import { allTabs, helpFor, matchesQuery, sectionFor } from "./navigation";
import { articles, findArticles } from "./help";
describe("1.2 navigation and offline documentation", () => {
  it("keeps all 0.9 tools and adds optimization and drawing workspaces", () => {
    expect(allTabs.map((t) => t[0]).sort()).toEqual(
      [
        "spatial",
        "spots",
        "synthesis",
        "coatings",
        "quality_report",
        "focus",
        "aperture",
        "goals",
        "pupil",
        "groups",
        "motion",
        "versions",
        "prescription",
        "geometry",
        "aberration",
        "wave",
        "preview",
        "optimization",
        "drawing",
      ].sort(),
    );
    expect(new Set(allTabs.map((t) => t[0])).size).toBe(allTabs.length);
    for (const [id] of allTabs)
      expect(articles.some((a) => a.id === helpFor(id))).toBe(true);
    expect(sectionFor("wave").id).toBe("quality");
  });
  it("bundles complete articles and resolves every local manual link", () => {
    expect(articles).toHaveLength(26);
    for (const a of articles) {
      expect(a.title).not.toBe("");
      expect(a.body.length).toBeGreaterThan(300);
      for (const m of a.body.matchAll(/\]\(([^)]+)\)/g))
        if (!m[1].startsWith("https://"))
          expect(
            articles.some((t) => t.id === m[1].replace(/\.md(?:#.*)?$/, "")),
          ).toBe(true);
    }
  });
  it("searches Chinese full text and normalized English with AND terms", () => {
    expect(matchesQuery("PSF / MTF 数值收敛", "ｍｔｆ 收敛")).toBe(true);
    expect(
      findArticles("无法采纳").some((a) => a.id === "troubleshooting"),
    ).toBe(true);
    expect(findArticles("角落").some((a) => a.id === "fields")).toBe(true);
    expect(findArticles("SHA-256").some((a) => a.id === "drawing")).toBe(true);
    expect(findArticles("abc-no-such-article")).toEqual([]);
  });
});
