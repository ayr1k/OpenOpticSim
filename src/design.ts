import { validCoverage } from "./fields";
import type {
  ApertureOptions,
  ApertureScan,
  OpticalGroup,
  Project,
} from "./types";
export const opticalKey = (p: Project) =>
  JSON.stringify(
    {
      materials: p.materials,
      surfaces: p.surfaces,
      stop: p.stop,
      object_distance: p.object_distance,
      sensor: p.sensor,
      fields: p.fields,
      field_spec: p.field_spec,
      wavelengths: p.wavelengths,
    },
    (_key, value) =>
      value && typeof value === "object" && !Array.isArray(value)
        ? Object.fromEntries(
            Object.keys(value)
              .sort()
              .map((k) => [k, value[k]]),
          )
        : value,
  );
export const defaultAperture: ApertureOptions = {
  f_numbers: [4, 5.6, 8, 11, 16, 22],
  refocus: false,
  sampling: 17,
  mtf: false,
};
// Candidate validity includes specifications and optimization bounds, not only rays.
export const candidateKey = (p: Project) =>
  opticalKey(p) +
  JSON.stringify([
    p.schema_version,
    p.name,
    p.description,
    p.workspace?.design_task,
    p.workspace?.constraints,
    p.workspace?.optimization,
  ]);
export function validAperture(o: ApertureOptions) {
  return (
    !!o &&
    Array.isArray(o.f_numbers) &&
    o.f_numbers.length > 0 &&
    o.f_numbers.length <= 12 &&
    o.f_numbers.every((f) => Number.isFinite(f) && f >= 0.5 && f <= 128) &&
    typeof o.refocus === "boolean" &&
    typeof o.mtf === "boolean" &&
    (o.wave_method === undefined ||
      ["fast", "sphere_kirchhoff"].includes(o.wave_method)) &&
    Number.isInteger(o.sampling) &&
    o.sampling >= 7 &&
    o.sampling <= 33 &&
    o.sampling % 2 === 1
  );
}
export function validGroups(p: Project, groups: OpticalGroup[]) {
  if (!Array.isArray(groups) || groups.length > 20) return false;
  return groups.every(
    (g, i) =>
      g &&
      typeof g.id === "string" &&
      g.id.length > 0 &&
      g.id.length <= 120 &&
      typeof g.name === "string" &&
      g.name.trim().length > 0 &&
      g.name.length <= 120 &&
      Number.isInteger(g.start) &&
      Number.isInteger(g.end) &&
      g.start >= 0 &&
      g.end > g.start &&
      g.end < p.surfaces.length &&
      p.surfaces[g.start].material !== 0 &&
      (g.start === 0 || p.surfaces[g.start - 1].material === 0) &&
      p.surfaces[g.end].material === 0 &&
      !groups
        .slice(0, i)
        .some((a) => a.id === g.id || (a.start <= g.end && g.start <= a.end)),
  );
}
export function validScan(s: ApertureScan) {
  if (
    !s ||
    !validAperture(s.options) ||
    !Array.isArray(s.points) ||
    s.points.length !== s.options.f_numbers.length ||
    !s.source ||
    !Array.isArray(s.source.fields)
  )
    return false;
  if (
    s.best_index !== null &&
    (!Number.isInteger(s.best_index) ||
      s.best_index < 0 ||
      s.best_index >= s.points.length)
  )
    return false;
  if (s.best_index !== null && s.points[s.best_index]?.score_um == null)
    return false;
  return s.points.every(
    (p, i) =>
      p &&
      validCoverage(p.coverage) &&
      (p.fields_rad === undefined ||
        (Array.isArray(p.fields_rad) &&
          p.fields_rad.length <= s.source.fields.length &&
          p.fields_rad.every(
            (v) => Number.isFinite(v) && Math.abs(v) <= 0.7,
          ))) &&
      p.requested_f_number === s.options.f_numbers[i] &&
      Number.isFinite(p.image_distance_mm) &&
      p.image_distance_mm > 0 &&
      [p.actual_f_number, p.stop_radius_mm, p.score_um].every(
        (v) => v === null || (Number.isFinite(v) && v >= 0),
      ) &&
      Array.isArray(p.fields_rms_um) &&
      p.fields_rms_um.length === s.source.fields.length &&
      p.fields_rms_um.every(
        (v) => v === null || (Number.isFinite(v) && v >= 0),
      ) &&
      [p.failed, p.sampled, p.aiming_failed, p.blocked].every(
        (v) => Number.isInteger(v) && v >= 0,
      ) &&
      p.failed <= p.sampled &&
      p.aiming_failed + p.blocked <= p.failed &&
      (p.score_um === null ||
        (p.failed === 0 &&
          p.sampled > 0 &&
          p.fields_rms_um.every((v) => v !== null))) &&
      Array.isArray(p.mtf) &&
      p.mtf.length <= s.source.fields.length &&
      p.mtf.every(
        (m) =>
          m &&
          (m.method === undefined ||
            m.method === "fast" ||
            m.method === "sphere_kirchhoff") &&
          Number.isFinite(m.field) &&
          (m.values === null ||
            (m.values.length === 4 &&
              m.values.every(
                (v) =>
                  v === null || (Number.isFinite(v) && v >= 0 && v <= 1.000001),
              ))) &&
          (m.error === null || typeof m.error === "string"),
      ) &&
      (p.diagnostic === null || typeof p.diagnostic === "string"),
  );
}
export function scanProject(scan: ApertureScan, index: number): Project {
  const point = scan.points[index];
  if (!point || point.score_um === null || !point.stop_radius_mm)
    throw Error("该扫描点没有通过完整几何采样，不能生成候选");
  const p = structuredClone(scan.source);
  const s = p.surfaces[p.stop];
  s.semi_aperture = point.stop_radius_mm;
  if (s.mechanical)
    s.mechanical.semi_diameter = Math.max(
      s.mechanical.semi_diameter,
      point.stop_radius_mm,
    );
  p.surfaces.at(-1)!.thickness = point.image_distance_mm;
  return p;
}
