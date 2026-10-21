import type {
  AnalysisPreset,
  OptimizationOptions,
  Project,
  Variable,
  VariableSetting,
} from "./types";
import { fieldTargets } from "./fields";
export const presetLabels: Record<AnalysisPreset, string> = {
  center: "中心优先",
  balanced: "全画面均衡",
  edge: "边缘优先",
};
export function fieldWeights(fields: number[], preset: AnalysisPreset) {
  const max = Math.max(1e-12, ...fields.map(Math.abs));
  const weights = fields.map((f) =>
    preset === "center"
      ? 1 + 3 * (1 - Math.abs(f) / max) ** 2
      : preset === "edge"
        ? 1 + 3 * (f / max) ** 2
        : 1,
  );
  const sum = weights.reduce((a, b) => a + b, 0);
  return weights.map((w) => w / sum);
}
export function variableValue(p: Project, v: Variable): number | undefined {
  const s = p.surfaces[v.surface];
  return !s
    ? undefined
    : v.kind === "asphere"
      ? s.asphere[v.coefficient ?? -1]
      : v.kind === "aperture"
        ? v.surface === p.stop
          ? s.semi_aperture
          : undefined
        : s[v.kind];
}
export function variableLabel(v: Variable) {
  return `S${v.surface + 1} · ${v.kind === "curvature" ? "曲率" : v.kind === "aperture" ? "光阑半径" : v.kind === "thickness" ? "面后间隔 / 像距" : `A${4 + 2 * (v.coefficient ?? 0)}`}`;
}
export function variableUnit(v: Variable) {
  return v.kind === "curvature"
    ? "mm⁻¹"
    : v.kind === "thickness" || v.kind === "aperture"
      ? "mm"
      : `mm⁻${3 + 2 * (v.coefficient ?? 0)}`;
}
export const variableKey = (v: Variable) =>
  `${v.surface}:${v.kind}:${v.coefficient ?? ""}`;
export function defaultSetting(p: Project, v: Variable): VariableSetting {
  const x = variableValue(p, v) ?? 0,
    delta =
      v.kind === "curvature"
        ? Math.max(0.005, Math.abs(x) * 0.5)
        : v.kind === "thickness" || v.kind === "aperture"
          ? Math.max(1, Math.abs(x) * 0.5)
          : Math.max(Math.abs(x) * 0.5, 10 ** (-5 - 2 * (v.coefficient ?? 0)));
  return {
    variable: v,
    min:
      v.kind === "thickness" || v.kind === "aperture"
        ? Math.max(v.kind === "aperture" ? 0.001 : 0, x - delta)
        : x - delta,
    max: x + delta,
    locked: false,
  };
}
export function defaultOptimization(
  p: Project,
  selected = 1,
): OptimizationOptions {
  if (!p.surfaces.length)
    return { variables: [], preset: "balanced", max_iterations: 25 };
  const surface = Math.max(0, Math.min(selected, p.surfaces.length - 1));
  return {
    quality: {
      max_rms_um: Array(fieldTargets(p)?.length ?? p.fields.length).fill(null),
      max_degradation_percent: 0,
    },
    variables: [
      defaultSetting(p, { kind: "curvature", surface }),
      defaultSetting(p, { kind: "thickness", surface: p.surfaces.length - 1 }),
    ],
    preset: "balanced",
    max_iterations: 25,
  };
}
export function optimizationError(
  p: Project,
  o: OptimizationOptions,
  checkCurrent = true,
): string {
  const g = o?.quality;
  if (
    g !== undefined &&
    (!g ||
      (g.require_sensor_coverage !== undefined &&
        typeof g.require_sensor_coverage !== "boolean") ||
      !Array.isArray(g.max_rms_um) ||
      (g.max_rms_um.length !== 0 &&
        g.max_rms_um.length !== (fieldTargets(p)?.length ?? p.fields.length)) ||
      [
        ...g.max_rms_um,
        g.max_worst_rms_um,
        g.max_degradation_percent,
        g.max_distortion_percent,
        g.max_lateral_color_um,
      ].some((v) => v != null && (!Number.isFinite(v) || v < 0)) ||
      (g.min_geometric_fraction != null &&
        (!Number.isFinite(g.min_geometric_fraction) ||
          g.min_geometric_fraction < 0 ||
          g.min_geometric_fraction > 1)))
  )
    return "逐视场画质保护无效或行数与当前视场不匹配，请重新配置";
  if (
    [o?.staged, o?.refocus_each_stage].some(
      (v) => v !== undefined && typeof v !== "boolean",
    )
  )
    return "分阶段优化设置无效";
  if (
    !o ||
    !["center", "balanced", "edge"].includes(o.preset) ||
    !Number.isInteger(o.max_iterations) ||
    o.max_iterations < 1 ||
    o.max_iterations > 100 ||
    !Array.isArray(o.variables) ||
    o.variables.length > 100
  )
    return "优化设置无效";
  const keys = new Set<string>();
  for (const [i, s] of o.variables.entries()) {
    const v = s?.variable;
    if (
      !v ||
      !["curvature", "thickness", "asphere", "aperture"].includes(v.kind) ||
      (v.kind === "aperture" && v.surface !== p.stop) ||
      !Number.isInteger(v.surface) ||
      v.surface < 0 ||
      v.surface >= p.surfaces.length ||
      (v.kind === "asphere" &&
        (!Number.isInteger(v.coefficient) ||
          v.coefficient! < 0 ||
          v.coefficient! > 3)) ||
      (v.kind !== "asphere" && v.coefficient !== undefined) ||
      typeof s.locked !== "boolean" ||
      [s.min, s.max].some(
        (x) => x !== null && (typeof x !== "number" || !Number.isFinite(x)),
      ) ||
      (s.min !== null && s.max !== null && s.min >= s.max) ||
      keys.has(variableKey(v))
    )
      return `变量第 ${i + 1} 行无效或重复；下限须小于上限`;
    keys.add(variableKey(v));
    const x = variableValue(p, v)!;
    if (
      checkCurrent &&
      ((s.min !== null && x < s.min) || (s.max !== null && x > s.max))
    )
      return `${variableLabel(v)} 当前值超出上下限，请调整范围`;
  }
  const count = o.variables.filter((v) => !v.locked).length;
  return count > 32 ? "最多同时优化 32 个未锁定变量" : "";
}
