import { describe, it, expect } from "vitest";
import fs from "node:fs";
import { copy, projectFromJson, fromCsv, toCsv } from "./project";
import {
  defaultOptimization,
  defaultSetting,
  fieldWeights,
  optimizationError,
} from "./optimization";
import { addVersion, ensureWorkspace, restoreVersion } from "./versions";
import { adoptionBlockers } from "./ConstraintEditor";
import {
  applyGlass,
  glasses,
  glassNumbers,
  refractiveIndex,
} from "./materials";
import type { ConstraintCheck, Material } from "./types";
const fixture = () =>
  projectFromJson(
    fs.readFileSync("examples/mechanical-annulus.oos.json", "utf8"),
  );
describe("0.6 bounded optimization and transparent acceptance", () => {
  it("persists cross-surface variables, locks, bounds and presets through JSON/CSV and versions", () => {
    const p = fixture(),
      o = defaultOptimization(p);
    o.preset = "edge";
    o.variables.push({
      ...defaultSetting(p, { kind: "asphere", surface: 2, coefficient: 3 }),
      locked: true,
    });
    ensureWorkspace(p).optimization = o;
    const v = addVersion(p, p, "边缘方案");
    for (const reopened of [
      projectFromJson(JSON.stringify(p)),
      fromCsv(toCsv(p)),
    ])
      expect(reopened).toEqual(p);
    expect(v.analysis_preset).toBe("edge");
    expect(
      restoreVersion(p, v.prescription, v.analysis_preset).workspace!
        .optimization!.preset,
    ).toBe("edge");
  });
  it("retains out-of-bound editable drafts but rejects malformed variable settings", () => {
    const p = fixture(),
      o = defaultOptimization(p);
    ensureWorkspace(p).optimization = o;
    o.variables[0].min = 10;
    o.variables[0].max = 20;
    expect(optimizationError(p, o)).toContain("超出");
    expect(projectFromJson(JSON.stringify(p))).toEqual(p);
    for (const mutate of [
      (q: typeof o) => {
        q.variables[0].max = 0;
      },
      (q: typeof o) => {
        q.variables.push(copy(q.variables[0]));
      },
      (q: typeof o) => {
        q.variables[0].variable.surface = 500;
      },
      (q: typeof o) => {
        q.preset = "bad" as never;
      },
    ]) {
      const q = copy(p);
      mutate(q.workspace!.optimization!);
      expect(() => projectFromJson(JSON.stringify(q))).toThrow();
    }
  });
  it("lists a hidden failure even among many passing rows and never overrides a rejected engine decision", () => {
    const pass: ConstraintCheck = {
      name: "center_mm",
      surface: 0,
      actual: 2,
      limit: 1,
      relation: "min",
      passed: true,
    };
    const checks = [
      ...Array.from({ length: 80 }, () => copy(pass)),
      { ...pass, name: "efl_error_mm", surface: null, passed: false },
    ];
    expect(adoptionBlockers({ accepted: false, checks })).toEqual([
      "焦距绝对误差",
    ]);
    expect(adoptionBlockers({ accepted: true, checks })).toEqual([
      "焦距绝对误差",
    ]);
    expect(adoptionBlockers({ accepted: false, checks: [pass] })[0]).toContain(
      "不一致",
    );
    expect(adoptionBlockers({ accepted: true, checks: [pass] })).toEqual([]);
  });
  it("normalizes all preset weights and treats symmetric fields equally", () => {
    for (const p of ["center", "balanced", "edge"] as const) {
      const w = fieldWeights([0, 0.07, 0.1, -0.1], p);
      expect(w.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 14);
      expect(w.every((w) => w > 0)).toBe(true);
      expect(w[2]).toBe(w[3]);
    }
  });
  it("clears variable references when restoring a different surface count", () => {
    const p = fixture();
    ensureWorkspace(p).optimization = defaultOptimization(
      p,
      p.surfaces.length - 1,
    );
    const shorter = copy(p);
    shorter.surfaces.pop();
    shorter.stop = 0;
    const restored = restoreVersion(p, shorter, "center");
    expect(restored.workspace!.optimization!.variables).toEqual([]);
    expect(restored.workspace!.optimization!.preset).toBe("center");
    expect(projectFromJson(JSON.stringify(restored))).toEqual(restored);
  });
});
describe("traceable real glass snapshots", () => {
  it("matches all eight catalog reference nd and Vd values from independent supplied metadata", () => {
    expect(glasses).toHaveLength(33);
    for (const entry of glasses) {
      const g = entry as unknown as Material,
        { nd, vd } = glassNumbers(g);
      expect(nd).toBeCloseTo(entry.nd_reference, 5);
      expect(vd).toBeCloseTo(entry.vd_reference, 1);
      expect(refractiveIndex(g, g.range_nm[0] - 1)).toBeNull();
      expect(g.source).toContain("CC0");
    }
  });
  it("changes only selected glass body, preserves other users of shared material and reopens with exact coefficients", () => {
    const p = fixture(),
      old = copy(p),
      m = glasses[0] as unknown as Material;
    const face = p.surfaces.findIndex((s) => s.material !== 0);
    applyGlass(p, face, m);
    expect(p.materials.slice(0, old.materials.length)).toEqual(old.materials);
    expect(p.surfaces.filter((_, i) => i !== face)).toEqual(
      old.surfaces.filter((_, i) => i !== face),
    );
    const count = p.materials.length;
    applyGlass(p, face, m);
    expect(p.materials).toHaveLength(count);
    expect(projectFromJson(JSON.stringify(p))).toEqual(p);
    expect(() => applyGlass(p, 0, m)).toThrow();
    p.wavelengths[0].nm = 200;
    expect(() => applyGlass(p, face, m)).toThrow();
  });
});
