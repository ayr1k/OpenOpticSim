import type { Project } from "./types";
import { sag } from "./project";
export interface Pose {
  offset_mm: [number, number, number];
  rotation_rad: [number, number, number];
}
export interface SpatialOptions {
  poses: Pose[];
  detector: Pose;
  rays: { origin_mm: number[]; direction: number[]; wavelength_nm: number }[];
  threads: number;
  groups?: { start: number; end: number; pose: Pose }[];
}
export interface SpatialWorkspace {
  topology: string;
  options: SpatialOptions;
}
export const identityPose = (): Pose => ({
  offset_mm: [0, 0, 0],
  rotation_rad: [0, 0, 0],
});
export function rotatePoint(p: number[], r: number[]): number[] {
  const [x, y, z] = p,
    [a, b, c] = r;
  const y1 = y * Math.cos(a) - z * Math.sin(a),
    z1 = y * Math.sin(a) + z * Math.cos(a);
  const x2 = x * Math.cos(b) + z1 * Math.sin(b),
    z2 = -x * Math.sin(b) + z1 * Math.cos(b);
  return [
    x2 * Math.cos(c) - y1 * Math.sin(c),
    x2 * Math.sin(c) + y1 * Math.cos(c),
    z2,
  ];
}
export function spatialWireframes(p: Project, o: SpatialOptions): number[][][] {
  let z = 0;
  const vertices = p.surfaces.map((s) => {
    const v = z;
    z += s.thickness;
    return v;
  });
  const lines: number[][][] = [];
  p.surfaces.forEach((s, i) => {
    const pose = o.poses[i] ?? identityPose();
    const transform = (q: number[]) => {
      let v = rotatePoint(q, pose.rotation_rad).map(
        (x, j) => x + pose.offset_mm[j] + (j === 2 ? vertices[i] : 0),
      );
      for (const g of o.groups ?? [])
        if (i >= g.start && i <= g.end) {
          const pivot = vertices[g.start];
          v = rotatePoint([v[0], v[1], v[2] - pivot], g.pose.rotation_rad).map(
            (x, j) => x + g.pose.offset_mm[j] + (j === 2 ? pivot : 0),
          );
        }
      return v;
    };
    for (let curve = 0; curve < 3; curve++) {
      const line: number[][] = [];
      for (let k = 0; k <= 48; k++) {
        const t = k / 48,
          r = curve === 0 ? s.semi_aperture : (2 * t - 1) * s.semi_aperture;
        const h = sag(s, Math.abs(r));
        if (!Number.isFinite(h)) continue;
        const q =
          curve === 0
            ? [r * Math.cos(t * 2 * Math.PI), r * Math.sin(t * 2 * Math.PI), h]
            : curve === 1
              ? [r, 0, h]
              : [0, r, h];
        line.push(transform(q));
      }
      lines.push(line);
    }
  });
  lines.push(
    [
      [-1, -1],
      [1, -1],
      [1, 1],
      [-1, 1],
      [-1, -1],
    ].map(([x, y]) =>
      rotatePoint(
        [(x * p.sensor[0]) / 2, (y * p.sensor[1]) / 2, 0],
        o.detector.rotation_rad,
      ).map((v, j) => v + o.detector.offset_mm[j] + (j === 2 ? z : 0)),
    ),
  );
  return lines;
}
export const topology = (p: Project) =>
  JSON.stringify(p.surfaces.map((s) => s.name));
export function defaultSpatial(p: Project): SpatialWorkspace {
  const radius = Math.min(2, (p.surfaces[0]?.semi_aperture ?? 1) * 0.5);
  // Explicit launch plane safely before the first vertex; not a pupil-aimed bundle.
  return {
    topology: topology(p),
    options: {
      poses: p.surfaces.map(identityPose),
      detector: identityPose(),
      threads: 1,
      rays: [-1, -0.5, 0, 0.5, 1].map((y) => ({
        origin_mm: [0, y * radius, -100],
        direction: [0, 0, 1],
        wavelength_nm: 587.5618,
      })),
    },
  };
}
export function validSpatial(s: SpatialWorkspace): boolean {
  const finite3 = (v: unknown) =>
    Array.isArray(v) &&
    v.length === 3 &&
    v.every((x) => typeof x === "number" && Number.isFinite(x));
  const pose = (p: Pose) =>
    !!p && finite3(p.offset_mm) && finite3(p.rotation_rad);
  return (
    !!s &&
    typeof s.topology === "string" &&
    !!s.options &&
    Array.isArray(s.options.poses) &&
    s.options.poses.length <= 100 &&
    s.options.poses.every(pose) &&
    pose(s.options.detector) &&
    Number.isInteger(s.options.threads) &&
    s.options.threads >= 1 &&
    s.options.threads <= 64 &&
    (s.options.groups === undefined ||
      (Array.isArray(s.options.groups) &&
        s.options.groups.length <= 100 &&
        s.options.groups.every(
          (g) =>
            Number.isInteger(g.start) &&
            Number.isInteger(g.end) &&
            g.start >= 0 &&
            g.end >= g.start &&
            g.end < s.options.poses.length &&
            pose(g.pose),
        ))) &&
    Array.isArray(s.options.rays) &&
    s.options.rays.length > 0 &&
    s.options.rays.length <= 1000 &&
    s.options.rays.every(
      (r) =>
        finite3(r.origin_mm) &&
        finite3(r.direction) &&
        r.direction.some((x) => x !== 0) &&
        Number.isFinite(r.wavelength_nm),
    )
  );
}
