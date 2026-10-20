import type {
  MotionWorkspace,
  StudySettings,
  StudyWorkspace,
  StudyOptions,
  StudyResult,
} from "./types";
import { motionOptions } from "./motion";
import { validCoverage } from "./fields";

export const studyModes = {
  trajectory: "生成变焦补偿轨迹",
  scan: "全焦段扫描",
  optimize: "多配置联合优化",
  focus: "对焦距离扫描",
};
export function defaultStudy(m: MotionWorkspace): StudySettings {
  return {
    mode: m.configurations.length > 1 ? "scan" : "focus",
    steps: 5,
    max_points: 41,
    adaptive: true,
    refine_rms_um: 3,
    driver_group: 0,
    compensator_group: m.groups.length > 1 ? 1 : 0,
    near_mm: 10000,
    far_mm: 100000,
    include_infinity: true,
    verification_sampling: Math.max(17, m.sampling),
    max_degradation_percent: 0,
    efl_tolerance_percent: null,
    min_glass_mm: 0,
    require_sensor_coverage: false,
    variables: [],
    iterations: 8,
  };
}
// Saved records and application revisions are excluded: they are not optical inputs.
export function studyKey(m: MotionWorkspace) {
  return JSON.stringify({
    reference: m.reference,
    groups: m.groups,
    configurations: m.configurations,
    min_air_mm: m.min_air_mm,
    sampling: m.sampling,
    criterion: m.criterion,
    max_rms_um: m.max_rms_um,
  });
}
export function studyOptions(
  m: MotionWorkspace,
  settings: StudySettings,
  selected: number,
): StudyOptions {
  const configurations =
    settings.mode === "focus" ? [m.configurations[selected]] : m.configurations;
  if (!configurations[0]) throw Error("请先选择有效配置");
  return {
    ...settings,
    motion: motionOptions(m, configurations[0]),
    configurations,
  };
}
const finite = (n: unknown): n is number =>
  typeof n === "number" && Number.isFinite(n);
export function validStudy(s: StudyWorkspace | undefined): boolean {
  if (s === undefined) return true;
  const v = s?.settings;
  if (
    !v ||
    !Object.hasOwn(studyModes, v.mode) ||
    ![
      v.steps,
      v.max_points,
      v.driver_group,
      v.compensator_group,
      v.verification_sampling,
      v.iterations,
    ].every(Number.isInteger) ||
    v.steps < 2 ||
    v.steps > 21 ||
    v.max_points < 2 ||
    v.max_points > 101 ||
    v.driver_group < 0 ||
    v.compensator_group < 0 ||
    v.verification_sampling < 17 ||
    v.verification_sampling > 33 ||
    v.verification_sampling % 2 !== 1 ||
    v.iterations < 1 ||
    v.iterations > 30 ||
    ![
      v.near_mm,
      v.far_mm,
      v.refine_rms_um,
      v.max_degradation_percent,
      v.min_glass_mm,
    ].every(finite) ||
    v.near_mm <= 1 ||
    v.far_mm < v.near_mm ||
    v.refine_rms_um <= 0 ||
    v.max_degradation_percent < 0 ||
    v.min_glass_mm < 0 ||
    (v.efl_tolerance_percent !== null &&
      (!finite(v.efl_tolerance_percent) || v.efl_tolerance_percent <= 0)) ||
    typeof v.adaptive !== "boolean" ||
    typeof v.include_infinity !== "boolean" ||
    typeof v.require_sensor_coverage !== "boolean" ||
    !Array.isArray(v.variables) ||
    v.variables.length > 12 ||
    v.variables.some(
      (x) =>
        !x ||
        !finite(x.min) ||
        !finite(x.max) ||
        x.min >= x.max ||
        (x.kind === "position"
          ? !Number.isInteger(x.configuration) ||
            x.configuration < 0 ||
            !Number.isInteger(x.group) ||
            x.group < 0
          : !["curvature", "thickness"].includes(x.kind) ||
            !Number.isInteger(x.surface) ||
            x.surface < 0),
    )
  )
    return false;
  if (!s.saved) return true;
  const r = s.saved.result;
  return (
    typeof s.saved.source_key === "string" &&
    s.saved.source_key.length < 300000 &&
    validStudy({ settings: s.saved.settings }) &&
    !!r &&
    Object.hasOwn(studyModes, r.mode) &&
    typeof r.accepted === "boolean" &&
    typeof r.note === "string" &&
    finite(r.sensor_reference_mm) &&
    Number.isInteger(r.verification_sampling) &&
    !!r.reference &&
    !("workspace" in r.reference) &&
    Array.isArray(r.configurations) &&
    r.configurations.length <= 101 &&
    [r.points, r.before].every(
      (rows) =>
        Array.isArray(rows) &&
        rows.length <= 101 &&
        rows.every(
          (p) =>
            p &&
            finite(p.coordinate) &&
            typeof p.accepted === "boolean" &&
            typeof p.diagnostic === "string" &&
            validCoverage(p.coverage) &&
            p.configuration &&
            Array.isArray(p.configuration.positions_mm) &&
            p.configuration.positions_mm.every(finite) &&
            Array.isArray(p.fields_rms_um) &&
            p.fields_rms_um.length <= 15 &&
            p.fields_rms_um.every((x) => x === null || finite(x)) &&
            Array.isArray(p.field_angles_rad) &&
            p.field_angles_rad.every(finite) &&
            [
              p.rms_um,
              p.efl_mm,
              p.f_number,
              p.distortion_percent,
              p.lateral_color_um,
              p.geometric_fraction,
              p.min_air_mm,
              p.min_glass_mm,
            ].every((x) => x === null || finite(x)),
        ),
    )
  );
}
export function studyCsv(result: StudyResult) {
  const quote = (x: unknown) => `"${String(x ?? "").replaceAll('"', '""')}"`;
  const header = [
    "position",
    "configuration",
    "object_distance_mm",
    "efl_mm",
    "f_number",
    "rms_um",
    "distortion_abs_percent",
    "lateral_color_um",
    "geometric_fraction",
    "min_air_sampled_mm",
    "min_glass_sampled_mm",
    "accepted",
    "diagnostic",
    ...(result.points[0]?.configuration.positions_mm.map(
      (_, i) => `group_${i + 1}_mm`,
    ) ?? []),
  ];
  return [
    header,
    ...result.points.map((p) => [
      p.coordinate,
      p.configuration.name,
      p.configuration.object_distance ?? "infinity",
      p.efl_mm,
      p.f_number,
      p.rms_um,
      p.distortion_percent,
      p.lateral_color_um,
      p.geometric_fraction,
      p.min_air_mm,
      p.min_glass_mm,
      p.accepted,
      p.diagnostic,
      ...p.configuration.positions_mm,
    ]),
  ]
    .map((r) => r.map(quote).join(","))
    .join("\n");
}
