import { expect, it } from "vitest";
import { glasses, glassNumbers, achromatPowers } from "./materials";
import { validCoating } from "./coatings";
import { projectFromJson } from "./project";
import { blankProject } from "./lenses";
import example from "../examples/400mm-repaired.oos.json";
import type { Project } from "./types";
import { prescription } from "./versions";
it("checks all expanded dispersion snapshots against independent catalog nd/Vd", () => {
  expect(glasses).toHaveLength(33);
  for (const m of glasses) {
    const g = glassNumbers(m);
    expect(Math.abs(g.nd! - m.nd_reference)).toBeLessThan(0.00002);
    expect(Math.abs(g.vd! - m.vd_reference)).toBeLessThan(0.03);
    expect(m.sha256).toMatch(/^[a-f0-9]{64}$/);
  }
  const p = achromatPowers(100, 64.17, 36.37)!;
  expect(p.positive + p.negative).toBeCloseTo(0.01, 12);
  expect(p.positive / 64.17 + p.negative / 36.37).toBeCloseTo(0, 12);
});
it("rejects malformed coating and preserves layers in saved optical snapshots", () => {
  expect(validCoating([{ name: "L", index: 1.38, thickness_nm: 100 }])).toBe(
    true,
  );
  expect(validCoating([{ name: "L", index: 1.38, thickness_nm: -1 }])).toBe(
    false,
  );
  const p = blankProject(example as Project, "coating");
  p.surfaces = [
    {
      name: "plane",
      curvature: 0,
      conic: 0,
      asphere: [0, 0, 0, 0],
      thickness: 10,
      semi_aperture: 1,
      material: 0,
      coating: [{ name: "L", index: 1.38, thickness_nm: 100 }],
    },
  ];
  expect(projectFromJson(JSON.stringify(p)).surfaces[0].coating).toEqual(
    p.surfaces[0].coating,
  );
  expect(prescription(p).surfaces[0].coating).toEqual(p.surfaces[0].coating);
  p.surfaces[0].coating![0].index = NaN;
  expect(() => projectFromJson(JSON.stringify(p))).toThrow();
});
