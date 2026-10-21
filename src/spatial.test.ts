import { describe, it, expect } from "vitest";
import { defaultSpatial, validSpatial, topology } from "./spatial";
import { projectFromJson } from "./project";
import singlet from "../examples/singlet.oos.json";
describe("spatial experiment persistence", () => {
  it("round trips poses, rays, detector and thread policy without changing optics", () => {
    const p = projectFromJson(JSON.stringify(singlet)),
      before = JSON.stringify(p.surfaces);
    const spatial = defaultSpatial(p);
    spatial.options.poses[0].offset_mm[0] = 0.25;
    spatial.options.detector.rotation_rad[1] = 0.01;
    p.workspace = { schema_version: 1, versions: [], spatial };
    expect(projectFromJson(JSON.stringify(p)).workspace?.spatial).toEqual(
      spatial,
    );
    expect(JSON.stringify(p.surfaces)).toBe(before);
    expect(validSpatial(spatial)).toBe(true);
    p.surfaces.reverse();
    expect(topology(p)).not.toBe(spatial.topology);
  });
  it("rejects zero directions and malformed poses on reopening", () => {
    const p = projectFromJson(JSON.stringify(singlet));
    const spatial = defaultSpatial(p);
    spatial.options.rays[0].direction = [0, 0, 0];
    p.workspace = { schema_version: 1, versions: [], spatial };
    expect(() => projectFromJson(JSON.stringify(p))).toThrow("三维");
  });
});
