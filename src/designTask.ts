import type { Candidate, Project } from "./types";
import { defaultConstraints, ensureWorkspace } from "./versions";
import { defaultOptimization } from "./optimization";
export interface DesignBrief {
  exploration?: "fixed" | "expanded";
  element_glass?: boolean;
  max_attempts?: number;
  min_center_mm?: number;
  min_edge_mm?: number;
  structure: string;
  focal_length_mm: number;
  f_number: number;
  sensor_mm: [number, number];
  half_field_deg: number;
  max_length_mm: number;
  max_rms_um: number;
  iterations: number;
  starts: number;
  require_sensor_coverage: boolean;
  glass_search: boolean;
}
export const defaultTask: DesignBrief = {
  exploration: "fixed",
  element_glass: false,
  max_attempts: 36,
  min_center_mm: 0.5,
  min_edge_mm: 0.1,
  structure: "tessar",
  focal_length_mm: 50,
  f_number: 8,
  sensor_mm: [36, 24],
  half_field_deg: 5,
  max_length_mm: 100,
  max_rms_um: 25,
  iterations: 25,
  starts: 1,
  require_sensor_coverage: true,
  glass_search: false,
};
export function projectFromCandidate(
  candidate: Candidate,
  task: DesignBrief,
): Project {
  if (!candidate.accepted || candidate.checks.some((c) => !c.passed))
    throw Error("候选尚未通过验收");
  const p = structuredClone(candidate.project),
    w = ensureWorkspace(p);
  const limit = (name: string) =>
    candidate.checks.find((c) => c.name === name)?.limit ?? 0;
  w.design_task = structuredClone(task);
  w.constraints = {
    target_efl_mm: task.focal_length_mm,
    efl_tolerance_mm: task.focal_length_mm * 0.01,
    max_length_mm: task.max_length_mm,
    min_center_mm: limit("center_mm"),
    min_edge_mm: limit("body_min_mm"),
    min_air_mm: limit("air_min_mm"),
    design: {
      f_number: task.f_number,
      f_tolerance: task.f_number * 0.03,
      min_bfl_mm: 0.01,
      max_front_diameter_mm: 1000,
    },
  };
  w.optimization = {
    ...defaultOptimization(p),
    variables: structuredClone(candidate.report?.variables ?? []),
    max_iterations: task.iterations,
    quality: {
      max_rms_um: [],
      max_worst_rms_um: task.max_rms_um,
      require_sensor_coverage: task.require_sensor_coverage,
    },
  };
  p.schema_version = Math.max(3, p.schema_version);
  return p;
}
export function validTask(b: DesignBrief) {
  return (
    !!b &&
    (b.exploration === undefined ||
      ["fixed", "expanded"].includes(b.exploration)) &&
    (b.element_glass === undefined || typeof b.element_glass === "boolean") &&
    Number.isInteger(b.max_attempts ?? 36) &&
    (b.max_attempts ?? 36) >= 1 &&
    (b.max_attempts ?? 36) <= 240 &&
    Number.isFinite(b.min_center_mm ?? 0.5) &&
    (b.min_center_mm ?? 0.5) >= 0.01 &&
    (b.min_center_mm ?? 0.5) <= 100 &&
    Number.isFinite(b.min_edge_mm ?? 0.1) &&
    (b.min_edge_mm ?? 0.1) >= 0 &&
    (b.min_edge_mm ?? 0.1) <= (b.min_center_mm ?? 0.5) &&
    ["auto", "tessar", "planar", "double_gauss", "sonnar"].includes(
      b.structure,
    ) &&
    [
      b.focal_length_mm,
      b.f_number,
      b.half_field_deg,
      b.max_length_mm,
      b.max_rms_um,
    ].every(Number.isFinite) &&
    b.focal_length_mm >= 10 &&
    b.focal_length_mm <= 1000 &&
    b.f_number >= 1 &&
    b.f_number <= 64 &&
    b.half_field_deg >= 0 &&
    b.half_field_deg <= 40 &&
    b.max_length_mm > 0 &&
    b.max_rms_um > 0 &&
    Array.isArray(b.sensor_mm) &&
    b.sensor_mm.length === 2 &&
    b.sensor_mm.every((v) => Number.isFinite(v) && v > 0 && v <= 100) &&
    Number.isInteger(b.iterations) &&
    b.iterations >= 1 &&
    b.iterations <= 100 &&
    Number.isInteger(b.starts) &&
    b.starts >= 1 &&
    b.starts <= 3 &&
    typeof b.require_sensor_coverage === "boolean" &&
    typeof b.glass_search === "boolean"
  );
}
// A task is a specification. Applying it to the current optical model is explicit.
export function applyTask(p: Project, task: DesignBrief) {
  if (!validTask(task)) throw Error("设计任务设置无效");
  const w = ensureWorkspace(p);
  w.design_task = structuredClone(task);
  w.constraints = {
    ...(w.constraints ?? defaultConstraints(p)),
    min_center_mm: task.min_center_mm ?? 0.5,
    min_edge_mm: task.min_edge_mm ?? 0.1,
    target_efl_mm: task.focal_length_mm,
    efl_tolerance_mm: task.focal_length_mm * 0.01,
    max_length_mm: task.max_length_mm,
    design: {
      ...(w.constraints?.design ?? {
        min_bfl_mm: 0.01,
        max_front_diameter_mm: 1000,
      }),
      f_number: task.f_number,
      f_tolerance: task.f_number * 0.03,
    },
  };
  const o = w.optimization ?? defaultOptimization(p);
  o.quality = {
    ...o.quality,
    max_rms_um: [],
    max_worst_rms_um: task.max_rms_um,
    require_sensor_coverage: task.require_sensor_coverage,
  };
  w.optimization = o;
  p.sensor = [...task.sensor_mm];
  p.field_spec = task.require_sensor_coverage ? { mode: "sensor" } : undefined;
  p.fields = [
    0,
    ((task.half_field_deg * Math.PI) / 180) * 0.7,
    (task.half_field_deg * Math.PI) / 180,
  ];
  if (task.require_sensor_coverage) p.fields = [0, 0, 0, 0];
  p.schema_version = Math.max(3, p.schema_version);
}
